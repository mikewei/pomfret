use std::env;

/// Snapshot of common proxy environment variables.
///
/// Values are pre-redacted and HTML-escaped only when rendering UI.
#[derive(Debug, Clone, Default)]
pub struct ProxyEnvInfo {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub all_proxy: Option<String>,
    pub no_proxy: Option<String>,
}

pub fn collect_proxy_env() -> ProxyEnvInfo {
    ProxyEnvInfo {
        // Environment variable names are case-sensitive on Linux.
        // Support both upper-case and curl-style lower-case variants.
        http_proxy: env_opt_any(&["HTTP_PROXY", "http_proxy"]),
        https_proxy: env_opt_any(&["HTTPS_PROXY", "https_proxy"]),
        all_proxy: env_opt_any(&["ALL_PROXY", "all_proxy"]),
        no_proxy: env_opt_any(&["NO_PROXY", "no_proxy"]),
    }
}

fn env_opt_any(var_names: &[&str]) -> Option<String> {
    for var_name in var_names {
        if let Some(v) = env_opt(var_name) {
            return Some(v);
        }
    }
    None
}

fn env_opt(var_name: &str) -> Option<String> {
    let raw = env::var(var_name).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(redact_proxy_value(trimmed))
}

/// Best-effort redaction of proxy credentials in URL-form strings.
///
/// Examples:
/// - `https://user:pass@host:1234/foo` -> `https://host:1234/foo`
/// - `http://host:1234` -> unchanged
pub fn redact_proxy_value(v: &str) -> String {
    // Only try to redact credentials in URL authority part (`scheme://...`).
    let (scheme, rest) = match v.split_once("://") {
        Some(x) => x,
        None => return v.to_string(),
    };

    let (authority, suffix) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };

    let at_pos = match authority.rfind('@') {
        Some(pos) => pos,
        None => return v.to_string(),
    };
    let redacted_authority = &authority[at_pos + 1..];
    format!("{scheme}://{redacted_authority}{suffix}")
}

/// Minimal HTML escape for injecting into `<code>` text nodes.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn build_proxy_panel_html(info: &ProxyEnvInfo) -> String {
    let any_set = info.http_proxy.is_some()
        || info.https_proxy.is_some()
        || info.all_proxy.is_some()
        || info.no_proxy.is_some();
    if !any_set {
        return String::new(); // hide whole panel when none set
    }

    let mut rows = String::new();

    if let Some(v) = &info.http_proxy {
        rows.push_str(&format!(
            r#"<div class="proxy-env-row"><span class="proxy-env-key" data-i18n="httpProxyLabel">HTTP_PROXY</span>=<code class="proxy-env-value">{}</code></div>"#,
            html_escape(v)
        ));
    }
    if let Some(v) = &info.https_proxy {
        rows.push_str(&format!(
            r#"<div class="proxy-env-row"><span class="proxy-env-key" data-i18n="httpsProxyLabel">HTTPS_PROXY</span>=<code class="proxy-env-value">{}</code></div>"#,
            html_escape(v)
        ));
    }
    if let Some(v) = &info.all_proxy {
        rows.push_str(&format!(
            r#"<div class="proxy-env-row"><span class="proxy-env-key" data-i18n="allProxyLabel">ALL_PROXY</span>=<code class="proxy-env-value">{}</code></div>"#,
            html_escape(v)
        ));
    }
    if let Some(v) = &info.no_proxy {
        rows.push_str(&format!(
            r#"<div class="proxy-env-row"><span class="proxy-env-key" data-i18n="noProxyLabel">NO_PROXY</span>=<code class="proxy-env-value">{}</code></div>"#,
            html_escape(v)
        ));
    }

    format!(
        r#"<section id="section-proxy" class="card">
  <h2 class="card-title" data-i18n="proxyTitle">Proxy settings</h2>
  <p class="card-desc" data-i18n="proxyDesc">Affects Pomfret outbound requests to backends.</p>
  <div class="proxy-env-list">{rows}</div>
</section>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_proxy_credentials_url_form() {
        let s = "https://user:pass@host:1234/foo?bar=baz";
        let redacted = redact_proxy_value(s);
        assert!(!redacted.contains("user:pass@"));
        assert_eq!(redacted, "https://host:1234/foo?bar=baz");
    }

    #[test]
    fn test_redact_no_scheme_no_change() {
        let s = "127.0.0.1:7890";
        assert_eq!(redact_proxy_value(s), s);
    }

    #[test]
    fn test_html_escape_basic() {
        assert_eq!(
            html_escape(r#"<&>"'"#),
            "&lt;&amp;&gt;&quot;&#39;"
        );
    }

    #[test]
    fn test_build_panel_hidden_when_none_set() {
        let info = ProxyEnvInfo::default();
        assert_eq!(build_proxy_panel_html(&info), "");
    }

    #[test]
    fn test_build_panel_escapes_and_redacts() {
        let raw = "https://user:pass@host:1234/<x>&y";
        let info = ProxyEnvInfo {
            https_proxy: Some(redact_proxy_value(raw)),
            ..Default::default()
        };
        let html = build_proxy_panel_html(&info);
        assert!(!html.contains("user:pass@"));
        assert!(html.contains("https://host:1234/&lt;x&gt;&amp;y"));
    }
}

