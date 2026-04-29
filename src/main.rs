//! Pomfret binary: Proxy Of Models For Routing, Evaluation & Telemetry — OpenAI-compatible proxy + web console.

use clap::Parser;
use pomfret::config::{default_backends_config_path, resolve_config, AppState};
use pomfret::proxy_env::collect_proxy_env;
use pomfret::routing::{default_routing_config_path, load_routing_config};
use pomfret::store::MemoryStore;
use pomfret::web::{router, NotifyEvent, ProviderPool, WebState};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "pomfret")]
#[command(about = "Proxy Of Models For Routing, Evaluation & Telemetry")]
struct Cli {
    /// Backends config file path (default: ~/.pomfret/backends.conf)
    #[arg(long, short = 'c')]
    config: Option<std::path::PathBuf>,

    /// Port to listen on (default: 8080)
    #[arg(long, short = 'p')]
    port: Option<u16>,

    /// Bind address (default: 127.0.0.1)
    #[arg(long, short = 'b')]
    bind: Option<String>,

    /// Timeout in seconds for outbound HTTP requests to each backend (default: 300)
    #[arg(long, default_value_t = 300)]
    backend_timeout_secs: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    // Print proxy-related environment variables at startup (redacted) so users
    // can quickly verify networking behavior for outbound backend requests.
    let proxy_env = collect_proxy_env();
    let any_proxy_set = proxy_env.http_proxy.is_some()
        || proxy_env.https_proxy.is_some()
        || proxy_env.all_proxy.is_some()
        || proxy_env.no_proxy.is_some();
    if any_proxy_set {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = proxy_env.http_proxy {
            parts.push(format!("http_proxy={v}"));
        }
        if let Some(v) = proxy_env.https_proxy {
            parts.push(format!("https_proxy={v}"));
        }
        if let Some(v) = proxy_env.all_proxy {
            parts.push(format!("all_proxy={v}"));
        }
        if let Some(v) = proxy_env.no_proxy {
            parts.push(format!("no_proxy={v}"));
        }

        tracing::info!(
            message = parts.join(" "),
            "proxy environment snapshot"
        );
    }

    let cli = Cli::parse();
    let resolved = resolve_config(
        cli.config.as_deref(),
        cli.port,
        cli.bind,
    )?;

    let backends_path = cli
        .config
        .clone()
        .unwrap_or_else(default_backends_config_path);
    let routing_path = default_routing_config_path();
    let routing_config = if routing_path.exists() {
        match load_routing_config(&routing_path) {
            Ok(rc) => {
                tracing::info!("loaded routing config from {}", routing_path.display());
                rc
            }
            Err(e) => {
                tracing::warn!("failed to load routing config: {}, using default", e);
                Default::default()
            }
        }
    } else {
        Default::default()
    };
    let app_state = AppState::new_with_routing(resolved.config, routing_config);
    let store = MemoryStore::new(500);
    let (notify_tx, _) = broadcast::channel::<NotifyEvent>(32);
    let web_state = WebState {
        app_state,
        store: store.clone(),
        backends_path,
        routing_path,
        notify_tx,
        provider_pool: ProviderPool::new(),
        backend_timeout_secs: cli.backend_timeout_secs,
    };

    let app = router(web_state);
    let addr: SocketAddr = format!("{}:{}", resolved.bind, resolved.port)
        .parse()
        .expect("valid bind and port");
    tracing::info!("listening on {}", addr);
    tracing::info!("Open web console at http://localhost:{}", resolved.port);
    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app.into_make_service(),
    )
    .await?;
    Ok(())
}
