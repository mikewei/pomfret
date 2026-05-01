//! Routing configuration: condition-based request routing to backends.
//!
//! Rules are evaluated top-to-bottom; the first matching rule determines the
//! target backend. A default target always exists as the final fallback.
//! Persisted to `~/.pomfret/routing.conf` (TOML).
//!
//! `model_matches` applies the pattern only to the request `model` string (not the body).

use crate::config::{AppState, BackendConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Condition type for a routing rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionType {
    Model,
    ModelMatches,
    Length,
    Regex,
}

/// How to select a backend when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingTarget {
    FirstAvailable,
    RoundRobin,
    Specific,
}

/// A single routing rule: if condition matches → route to target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub condition_type: ConditionType,
    pub condition_value: String,
    pub target: RoutingTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_backend_id: Option<String>,
}

/// Complete routing configuration (rules + default fallback).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub rules: Vec<RoutingRule>,
    pub default_target: RoutingTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_backend_id: Option<String>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            default_target: RoutingTarget::FirstAvailable,
            default_backend_id: None,
        }
    }
}

/// Default routing config path: `~/.pomfret/routing.conf`.
pub fn default_routing_config_path() -> std::path::PathBuf {
    crate::config::home_dir().join(".pomfret").join("routing.conf")
}

/// Load routing config from a TOML file.
pub fn load_routing_config(
    path: &Path,
) -> Result<RoutingConfig, Box<dyn std::error::Error + Send + Sync>> {
    let s = std::fs::read_to_string(path)?;
    let rc: RoutingConfig = toml::from_str(&s)?;
    Ok(rc)
}

/// Save routing config to a TOML file; creates parent directory if needed.
pub fn save_routing_config(
    path: &Path,
    config: &RoutingConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml = toml::to_string_pretty(config)?;
    std::fs::write(path, toml)?;
    Ok(())
}

/// Evaluate whether a single rule matches the given request.
fn rule_matches(
    rule: &RoutingRule,
    model: Option<&str>,
    body: Option<&str>,
    body_len: usize,
) -> bool {
    match rule.condition_type {
        ConditionType::Model => model
            .map(|m| m == rule.condition_value)
            .unwrap_or(false),
        ConditionType::ModelMatches => {
            if let Ok(re) = regex::Regex::new(&rule.condition_value) {
                model.map(|m| re.is_match(m)).unwrap_or(false)
            } else {
                false
            }
        },
        ConditionType::Length => rule
            .condition_value
            .parse::<usize>()
            .map(|threshold| body_len > threshold)
            .unwrap_or(false),
        ConditionType::Regex => {
            if let Ok(re) = regex::Regex::new(&rule.condition_value) {
                body.map(|b| re.is_match(b)).unwrap_or(false)
            } else {
                false
            }
        }
    }
}

/// Select a backend according to the given target strategy.
fn select_backend(
    backends: &[BackendConfig],
    target: &RoutingTarget,
    target_backend_id: Option<&str>,
    app_state: &AppState,
) -> Option<BackendConfig> {
    if backends.is_empty() {
        return None;
    }
    match target {
        RoutingTarget::FirstAvailable => backends.first().cloned(),
        RoutingTarget::RoundRobin => {
            let idx = app_state.next_round_robin() % backends.len();
            backends.get(idx).cloned()
        }
        RoutingTarget::Specific => {
            if let Some(id) = target_backend_id {
                backends.iter().find(|b| b.id == id).cloned()
            } else {
                backends.first().cloned()
            }
        }
    }
}

/// Resolve which backend to use for a request based on routing rules.
///
/// Evaluates rules top-to-bottom; returns the first match.
/// Falls back to default target if no rule matches.
/// `body` is the raw request JSON string, used for Regex matching against prompt content.
pub async fn resolve_backend(
    app_state: &AppState,
    model: Option<&str>,
    body: Option<&str>,
    body_len: usize,
) -> Option<BackendConfig> {
    let routing = app_state.get_routing_config().await;
    let backends = app_state.list_backends().await;

    if backends.is_empty() {
        return None;
    }

    for rule in &routing.rules {
        if rule_matches(rule, model, body, body_len) {
            if let Some(b) = select_backend(
                &backends,
                &rule.target,
                rule.target_backend_id.as_deref(),
                app_state,
            ) {
                return Some(b);
            }
        }
    }

    select_backend(
        &backends,
        &routing.default_target,
        routing.default_backend_id.as_deref(),
        app_state,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rule(condition_type: ConditionType, condition_value: &str) -> RoutingRule {
        RoutingRule {
            condition_type,
            condition_value: condition_value.to_string(),
            target: RoutingTarget::FirstAvailable,
            target_backend_id: None,
        }
    }

    #[test]
    fn model_matches_when_pattern_matches() {
        let rule = sample_rule(ConditionType::ModelMatches, r"^gpt-4");
        assert!(rule_matches(
            &rule,
            Some("gpt-4-turbo"),
            None,
            0
        ));
    }

    #[test]
    fn model_matches_when_pattern_does_not_match() {
        let rule = sample_rule(ConditionType::ModelMatches, r"^gpt-4");
        assert!(!rule_matches(&rule, Some("claude-3"), None, 0));
    }

    #[test]
    fn model_matches_false_without_model() {
        let rule = sample_rule(ConditionType::ModelMatches, r".*");
        assert!(!rule_matches(&rule, None, None, 0));
    }

    #[test]
    fn model_matches_invalid_regex_never_matches() {
        let rule = sample_rule(ConditionType::ModelMatches, r"(");
        assert!(!rule_matches(&rule, Some("anything"), None, 0));
    }
}
