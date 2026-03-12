//! Configuration and backend list.
//!
//! Backends are identified by id; one is selected as "current" for routing.
//! Server options (bind, port) are managed by CLI only; backends are loaded from
//! `backends.conf` (default: ~/.pomfret/backends.conf).

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Backend type: OpenAI-compatible or Ollama (same HTTP API, different base_url/key handling).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    #[serde(rename = "openai_compat", alias = "open_ai_compat")]
    OpenAiCompat,
    Ollama,
}

/// Single LLM backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_backend_type")]
    pub backend_type: BackendType,
    /// If set, override the model in every request forwarded to this backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

fn default_backend_type() -> BackendType {
    BackendType::OpenAiCompat
}

/// Root configuration (can be loaded from file/env later).
#[derive(Debug, Clone)]
pub struct Config {
    pub backends: Vec<BackendConfig>,
    /// Index into backends for "current" backend; None means no backend selected.
    pub current_index: Option<usize>,
}

impl Config {
    /// Default config with no backends (for tests / minimal run).
    pub fn default_empty() -> Self {
        Self {
            backends: Vec::new(),
            current_index: None,
        }
    }

    /// Default config with a single Ollama backend.
    /// Caller can replace with file/env config later.
    pub fn default_with_examples() -> Self {
        Self {
            backends: vec![BackendConfig {
                id: "ollama".to_string(),
                name: "Ollama".to_string(),
                base_url: "http://127.0.0.1:11434/v1".to_string(),
                api_key: None,
                backend_type: BackendType::Ollama,
                model: None,
            }],
            current_index: Some(0),
        }
    }

    /// Current backend config if one is selected.
    pub fn current_backend(&self) -> Option<&BackendConfig> {
        self.current_index.and_then(|i| self.backends.get(i))
    }

    /// Set current backend by index; returns false if index out of range.
    pub fn set_current_index(&mut self, index: Option<usize>) -> bool {
        match index {
            None => {
                self.current_index = None;
                true
            }
            Some(i) if i < self.backends.len() => {
                self.current_index = Some(i);
                true
            }
            _ => false,
        }
    }

    /// Update backend at index; returns false if index out of range.
    pub fn update_backend(
        &mut self,
        index: usize,
        name: Option<String>,
        base_url: Option<String>,
        api_key: Option<String>,
        backend_type: Option<BackendType>,
        model: Option<Option<String>>,
    ) -> bool {
        let Some(b) = self.backends.get_mut(index) else {
            return false;
        };
        if let Some(n) = name {
            b.name = n;
        }
        if let Some(u) = base_url {
            b.base_url = u;
        }
        if let Some(k) = api_key {
            b.api_key = Some(k);
        }
        if let Some(t) = backend_type {
            b.backend_type = t;
        }
        if let Some(m) = model {
            b.model = m;
        }
        true
    }

    /// Remove backend at index; adjusts current_index. Returns false if index out of range.
    pub fn delete_backend(&mut self, index: usize) -> bool {
        if index >= self.backends.len() {
            return false;
        }
        self.backends.remove(index);
        self.current_index = match self.current_index {
            None => None,
            Some(i) if i == index => {
                if self.backends.is_empty() {
                    None
                } else {
                    Some(0.min(i.saturating_sub(1)))
                }
            }
            Some(i) if i > index => Some(i - 1),
            Some(i) => Some(i),
        };
        true
    }

    /// Append a new backend; returns true.
    /// `backend_type`: if None, defaults to OpenAiCompat.
    pub fn add_backend(
        &mut self,
        name: String,
        base_url: String,
        api_key: Option<String>,
        backend_type: Option<BackendType>,
        model: Option<String>,
    ) -> bool {
        let id = Uuid::new_v4().to_string();
        self.backends.push(BackendConfig {
            id,
            name,
            base_url,
            api_key,
            backend_type: backend_type.unwrap_or(BackendType::OpenAiCompat),
            model,
        });
        true
    }
}

/// Configuration as stored in backends.conf (only backends and current selection).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct FileConfig {
    pub current_index: Option<usize>,
    pub backends: Option<Vec<BackendConfig>>,
}

/// Resolved server (CLI) + backends (file): bind/port from CLI, config from backends.conf.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub bind: String,
    pub port: u16,
    pub config: Config,
}

/// Returns default backends config path: `~/.pomfret/backends.conf`.
pub fn default_backends_config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".pomfret").join("backends.conf")
}

/// Serialize in-memory config to TOML string (for save/export to backends.conf).
pub fn config_to_toml(config: &Config) -> Result<String, toml::ser::Error> {
    let fc = FileConfig {
        current_index: config.current_index,
        backends: Some(config.backends.clone()),
    };
    toml::to_string_pretty(&fc)
}

/// Write config to path; create parent directory if needed.
pub fn write_config_to_path(
    path: &Path,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml = config_to_toml(config)?;
    std::fs::write(path, toml)?;
    Ok(())
}

/// Load config from a TOML file; returns error if file exists but parse fails.
pub fn load_file_config(path: &Path) -> Result<FileConfig, Box<dyn std::error::Error + Send + Sync>> {
    let s = std::fs::read_to_string(path)?;
    let fc = toml::from_str(&s)?;
    Ok(fc)
}

/// Apply backends file content into Config (file overrides defaults).
fn apply_file_config(config: &mut Config, file: &FileConfig) {
    if let Some(backends) = &file.backends {
        config.backends = backends.clone();
        config.current_index = file
            .current_index
            .filter(|&i| i < config.backends.len())
            .or(if config.backends.is_empty() {
                None
            } else {
                Some(0)
            });
    } else if let Some(idx) = file.current_index {
        if idx < config.backends.len() {
            config.current_index = Some(idx);
        }
    }
}

/// Resolve full config: bind/port from CLI (defaults 127.0.0.1, 8080); backends from file.
/// `backends_path`: if None, uses default `~/.pomfret/backends.conf`.
pub fn resolve_config(
    backends_path: Option<&std::path::Path>,
    cli_port: Option<u16>,
    cli_bind: Option<String>,
) -> Result<ResolvedConfig, Box<dyn std::error::Error + Send + Sync>> {
    let bind = match cli_bind {
        None => "127.0.0.1".to_string(),
        Some(s) if s == "0" => "0.0.0.0".to_string(),
        Some(s) => s,
    };
    let port = cli_port.unwrap_or(8080);
    let mut config = Config::default_with_examples();

    let default_path = default_backends_config_path();
    let path = backends_path.unwrap_or(&default_path);
    if path.exists() {
        let file = load_file_config(path)?;
        apply_file_config(&mut config, &file);
    }

    Ok(ResolvedConfig { bind, port, config })
}

/// Shared app state: config + current backend selection (thread-safe).
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Get current backend config (read).
    pub async fn current_backend(&self) -> Option<BackendConfig> {
        let c = self.config.read().await;
        c.current_backend().cloned()
    }

    /// Set current backend by index.
    pub async fn set_current_backend_index(&self, index: Option<usize>) -> bool {
        let mut c = self.config.write().await;
        c.set_current_index(index)
    }

    /// List all backends (for API/UI).
    pub async fn list_backends(&self) -> Vec<BackendConfig> {
        let c = self.config.read().await;
        c.backends.clone()
    }

    /// Current selected index.
    pub async fn current_backend_index(&self) -> Option<usize> {
        let c = self.config.read().await;
        c.current_index
    }

    /// Update backend at index.
    pub async fn update_backend(
        &self,
        index: usize,
        name: Option<String>,
        base_url: Option<String>,
        api_key: Option<String>,
        backend_type: Option<BackendType>,
        model: Option<Option<String>>,
    ) -> bool {
        let mut c = self.config.write().await;
        c.update_backend(index, name, base_url, api_key, backend_type, model)
    }

    /// Add a new backend.
    pub async fn add_backend(
        &self,
        name: String,
        base_url: String,
        api_key: Option<String>,
        backend_type: Option<BackendType>,
        model: Option<String>,
    ) -> bool {
        let mut c = self.config.write().await;
        c.add_backend(name, base_url, api_key, backend_type, model)
    }

    /// Delete backend at index.
    pub async fn delete_backend(&self, index: usize) -> bool {
        let mut c = self.config.write().await;
        c.delete_backend(index)
    }
}
