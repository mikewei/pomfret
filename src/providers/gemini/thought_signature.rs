//! Gemini `thought_signature`: cache by `tool_calls[].id` or `SIGID:` inside `<think>`…`</think>` in `content`,
//! strip from responses, inject on requests.

use crate::cache::LruTtlCache;
use bytes::Bytes;
use std::time::Duration;
use uuid::Uuid;

const MAX_ENTRIES: usize = 256;
const TTL: Duration = Duration::from_secs(3600);

/// Opening/closing think markers wrapping `SIGID:` in serialized `content`.
const THINK_OPEN: &str = concat!("<", "think", ">");
const THINK_CLOSE: &str = concat!("<", "/", "think", ">");

/// Server-side store for `thought_signature` keyed by OpenAI `tool_calls[].id` or `SIGID:` inside think tags.
#[derive(Clone)]
pub(crate) struct ThoughtSignatureCache {
    inner: LruTtlCache<String, String>,
}

impl Default for ThoughtSignatureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ThoughtSignatureCache {
    pub(crate) fn new() -> Self {
        Self {
            inner: LruTtlCache::new(MAX_ENTRIES, TTL),
        }
    }

    pub(crate) fn put(&self, call_id: impl Into<String>, signature: impl Into<String>) {
        let call_id = call_id.into();
        let signature = signature.into();
        if call_id.is_empty() || signature.is_empty() {
            return;
        }
        self.inner.put(call_id, signature);
    }

    pub(crate) fn get(&self, call_id: &str) -> Option<String> {
        if call_id.is_empty() {
            return None;
        }
        self.inner.get(&call_id.to_string())
    }
}

fn new_sig_id() -> String {
    format!("tsig_{}", Uuid::new_v4().as_simple())
}

fn thought_signature_present(obj: &serde_json::Value) -> bool {
    obj.pointer("/extra_content/google/thought_signature")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

fn extract_and_remove_sig(obj: &mut serde_json::Value) -> Option<String> {
    let before = serde_json::to_string(obj).ok();
    let sig = obj
        .pointer("/extra_content/google/thought_signature")
        .and_then(|v| v.as_str())
        .map(String::from)?;

    let mut remove_google = false;
    if let Some(g) = obj
        .pointer_mut("/extra_content/google")
        .and_then(|v| v.as_object_mut())
    {
        g.remove("thought_signature");
        remove_google = g.is_empty();
    }

    let mut remove_ec = false;
    if remove_google {
        if let Some(ec) = obj
            .pointer_mut("/extra_content")
            .and_then(|v| v.as_object_mut())
        {
            ec.remove("google");
            remove_ec = ec.is_empty();
        }
    }

    if remove_ec {
        if let Some(o) = obj.as_object_mut() {
            o.remove("extra_content");
        }
    }

    tracing::trace!(
        signature = %sig,
        before = before.as_deref().unwrap_or("<non-serializable>"),
        after = serde_json::to_string(obj).ok().as_deref().unwrap_or("<non-serializable>"),
        "tsig: extract_and_remove_sig transformed object"
    );

    Some(sig)
}

fn think_wrapped_sigid(sig_id: &str) -> String {
    format!("{}SIGID:{}{}", THINK_OPEN, sig_id, THINK_CLOSE)
}

/// Strip `<think>…SIGID:…</think>` blocks (collect ids), then plain `SIGID:` lines (legacy).
fn strip_think_sigids_and_plain_lines(content: &str) -> (String, Vec<String>) {
    let mut ids = Vec::new();
    let mut out = String::new();
    let mut rest = content;
    while let Some(i) = rest.find(THINK_OPEN) {
        out.push_str(&rest[..i]);
        let after_open = &rest[i + THINK_OPEN.len()..];
        if let Some(j) = after_open.find(THINK_CLOSE) {
            let inner = &after_open[..j];
            for line in inner.lines() {
                let t = line.trim();
                if let Some(r) = t.strip_prefix("SIGID:") {
                    let id = r.trim();
                    if !id.is_empty() {
                        ids.push(id.to_string());
                    }
                }
            }
            rest = &after_open[j + THINK_CLOSE.len()..];
        } else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);

    let mut kept: Vec<&str> = Vec::new();
    for line in out.lines() {
        if let Some(r) = line.strip_prefix("SIGID:") {
            let id = r.trim();
            if !id.is_empty() {
                ids.push(id.to_string());
            }
        } else {
            kept.push(line);
        }
    }
    let cleaned = kept.join("\n");
    if !ids.is_empty() || cleaned != content {
        tracing::trace!(
            before = %content,
            after = %cleaned,
            ids = ?ids,
            "tsig: stripped SIGID markers from content"
        );
    }
    (cleaned, ids)
}

fn append_sigid_to_content_container(obj: &mut serde_json::Value, sig_id: &str) {
    let block = think_wrapped_sigid(sig_id);
    let before_content = obj.get("content").cloned();
    match obj.get_mut("content") {
        None => {
            obj["content"] = serde_json::Value::String(block);
        }
        Some(serde_json::Value::Null) => {
            *obj.get_mut("content").expect("content") = serde_json::Value::String(block);
        }
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                *s = block;
            } else {
                s.push('\n');
                s.push_str(&block);
            }
        }
        Some(serde_json::Value::Array(parts)) => {
            if let Some(last) = parts.last_mut() {
                if let Some(obj) = last.as_object_mut() {
                    if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(serde_json::Value::String(t)) = obj.get_mut("text") {
                            if t.is_empty() {
                                *t = block;
                            } else {
                                t.push('\n');
                                t.push_str(&block);
                            }
                            return;
                        }
                    }
                }
            }
            parts.push(serde_json::json!({
                "type": "text",
                "text": format!("\n{}", block)
            }));
        }
        Some(_) => {
            obj["content"] = serde_json::Value::String(block);
        }
    }
    tracing::trace!(
        sig_id = %sig_id,
        before = before_content
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
            .as_deref()
            .unwrap_or("<none>"),
        after = obj
            .get("content")
            .and_then(|v| serde_json::to_string(v).ok())
            .as_deref()
            .unwrap_or("<none>"),
        "tsig: appended think-wrapped SIGID into content"
    );
}

fn cache_tool_call_signatures(obj: &mut serde_json::Value, cache: &ThoughtSignatureCache) -> bool {
    let mut modified = false;
    let Some(tcs) = obj
        .get_mut("tool_calls")
        .and_then(|v| v.as_array_mut())
    else {
        return false;
    };
    for tc in tcs.iter_mut() {
        let before_tc = serde_json::to_string(tc).ok();
        let Some(tc_obj) = tc.as_object_mut() else {
            continue;
        };
        let id = tc_obj
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let Some(id) = id else {
            continue;
        };
        if !thought_signature_present(tc) {
            continue;
        }
        if let Some(sig) = extract_and_remove_sig(tc) {
            cache.put(&id, sig);
            tracing::trace!(
                call_id = %id,
                before = before_tc.as_deref().unwrap_or("<non-serializable>"),
                after = serde_json::to_string(tc).ok().as_deref().unwrap_or("<non-serializable>"),
                "tsig: cached tool_call signature and stripped field"
            );
            modified = true;
        }
    }
    modified
}

fn cache_message_level_signature(obj: &mut serde_json::Value, cache: &ThoughtSignatureCache) -> bool {
    if !thought_signature_present(obj) {
        return false;
    }
    let Some(sig) = extract_and_remove_sig(obj) else {
        return false;
    };
    let before = serde_json::to_string(obj).ok();
    let id = new_sig_id();
    cache.put(&id, sig);
    append_sigid_to_content_container(obj, &id);
    tracing::trace!(
        sig_id = %id,
        before = before.as_deref().unwrap_or("<non-serializable>"),
        after = serde_json::to_string(obj).ok().as_deref().unwrap_or("<non-serializable>"),
        "tsig: cached message-level signature and embedded SIGID marker"
    );
    true
}

fn inject_message_level_from_content_sigids(
    msg: &mut serde_json::Value,
    cache: &ThoughtSignatureCache,
) {
    let mut last_sig: Option<String> = None;
    match msg.get_mut("content") {
        Some(serde_json::Value::String(s)) => {
            let owned = std::mem::take(s);
            let (cleaned, ids) = strip_think_sigids_and_plain_lines(&owned);
            tracing::trace!(
                before = %owned,
                after = %cleaned,
                ids = ?ids,
                "tsig: parsed SIGID markers from assistant string content"
            );
            for id in ids {
                if let Some(sig) = cache.get(&id) {
                    tracing::trace!(
                        sig_id = %id,
                        "tsig: resolved SIGID from cache for assistant message"
                    );
                    last_sig = Some(sig);
                } else {
                    tracing::trace!(
                        sig_id = %id,
                        "tsig: SIGID not found in cache for assistant message"
                    );
                }
            }
            *s = cleaned;
        }
        Some(serde_json::Value::Array(parts)) => {
            for part in parts.iter_mut() {
                let Some(p) = part.as_object_mut() else {
                    continue;
                };
                if p.get("type").and_then(|t| t.as_str()) != Some("text") {
                    continue;
                }
                let Some(serde_json::Value::String(t)) = p.get_mut("text") else {
                    continue;
                };
                let owned = std::mem::take(t);
                let (cleaned, ids) = strip_think_sigids_and_plain_lines(&owned);
                tracing::trace!(
                    before = %owned,
                    after = %cleaned,
                    ids = ?ids,
                    "tsig: parsed SIGID markers from assistant content text part"
                );
                for id in ids {
                    if let Some(sig) = cache.get(&id) {
                        tracing::trace!(
                            sig_id = %id,
                            "tsig: resolved SIGID from cache for assistant content part"
                        );
                        last_sig = Some(sig);
                    } else {
                        tracing::trace!(
                            sig_id = %id,
                            "tsig: SIGID not found in cache for assistant content part"
                        );
                    }
                }
                *t = cleaned;
            }
        }
        _ => {}
    }
    if let Some(sig) = last_sig {
        let before = serde_json::to_string(msg).ok();
        msg["extra_content"]["google"]["thought_signature"] = serde_json::Value::String(sig);
        tracing::trace!(
            before = before.as_deref().unwrap_or("<non-serializable>"),
            after = serde_json::to_string(msg).ok().as_deref().unwrap_or("<non-serializable>"),
            "tsig: injected message-level thought_signature from SIGID marker"
        );
    }
}

pub(crate) fn cache_signatures_from_response_value(
    val: &mut serde_json::Value,
    cache: &ThoughtSignatureCache,
) -> bool {
    let mut modified = false;
    let Some(choices) = val.get_mut("choices").and_then(|c| c.as_array_mut()) else {
        return false;
    };
    for choice in choices.iter_mut() {
        let Some(message) = choice.get_mut("message") else {
            continue;
        };
        if cache_tool_call_signatures(message, cache) {
            modified = true;
        }
        if cache_message_level_signature(message, cache) {
            modified = true;
        }
    }
    modified
}

fn transform_sse_line_cache_strip(line: &str, cache: &ThoughtSignatureCache) -> String {
    let data = match line
        .strip_prefix("data: ")
        .or_else(|| line.strip_prefix("data:"))
    {
        Some(d) => d.trim(),
        None => return line.to_string(),
    };
    if data == "[DONE]" {
        return line.to_string();
    }
    let mut chunk: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return line.to_string(),
    };

    let mut modified = false;

    if let Some(choices) = chunk.get_mut("choices").and_then(|v| v.as_array_mut()) {
        for choice in choices.iter_mut() {
            let delta = match choice.get_mut("delta") {
                Some(d) => d,
                None => continue,
            };
            if cache_tool_call_signatures(delta, cache) {
                modified = true;
            }
            if cache_message_level_signature(delta, cache) {
                modified = true;
            }
        }
    }

    if modified {
        let out = format!(
            "data: {}",
            serde_json::to_string(&chunk).unwrap_or_else(|_| data.to_string())
        );
        tracing::trace!(
            before = %line,
            after = %out,
            "tsig: transformed SSE data line"
        );
        out
    } else {
        line.to_string()
    }
}

pub(crate) fn transform_sse_chunk_cache_strip(
    chunk: &[u8],
    pending: &mut Vec<u8>,
    cache: &ThoughtSignatureCache,
) -> Bytes {
    let mut combined = std::mem::take(pending);
    combined.extend_from_slice(chunk);

    let (complete_bytes, remaining) =
        if let Some(last_nl) = combined.iter().rposition(|&b| b == b'\n') {
            (&combined[..=last_nl], &combined[last_nl + 1..])
        } else {
            *pending = combined;
            return Bytes::new();
        };
    *pending = remaining.to_vec();

    let complete_str = String::from_utf8_lossy(complete_bytes);
    if !complete_str.contains("thought_signature") {
        return Bytes::from(complete_bytes.to_vec());
    }

    let mut output = String::with_capacity(complete_str.len());
    for line in complete_str.lines() {
        if !line.is_empty() {
            output.push_str(&transform_sse_line_cache_strip(line, cache));
        }
        output.push('\n');
    }
    Bytes::from(output.into_bytes())
}

pub(crate) fn inject_cached_signatures_into_gemini_request(
    body: &mut serde_json::Value,
    cache: &ThoughtSignatureCache,
) {
    let Some(msgs) = body.get_mut("messages").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for msg in msgs.iter_mut() {
        if msg.get("role").and_then(|r| r.as_str()) != Some("assistant") {
            continue;
        }
        inject_message_level_from_content_sigids(msg, cache);
        let Some(tcs) = msg.get_mut("tool_calls").and_then(|v| v.as_array_mut()) else {
            continue;
        };
        for tc in tcs.iter_mut() {
            let before_tc = serde_json::to_string(tc).ok();
            let id = match tc.get("id").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let Some(sig) = cache.get(&id) else {
                continue;
            };
            tc["extra_content"]["google"]["thought_signature"] =
                serde_json::Value::String(sig);
            tracing::trace!(
                call_id = %id,
                before = before_tc.as_deref().unwrap_or("<non-serializable>"),
                after = serde_json::to_string(tc).ok().as_deref().unwrap_or("<non-serializable>"),
                "tsig: injected tool_call thought_signature from cache"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_non_streaming_strips_when_id_present() {
        let cache = ThoughtSignatureCache::new();
        let mut body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "extra_content": { "google": { "thought_signature": "SIG_A" } },
                        "function": { "name": "f", "arguments": "{}" },
                        "id": "fc-1",
                        "type": "function"
                    }]
                }
            }]
        });
        assert!(cache_signatures_from_response_value(&mut body, &cache));
        assert!(body["choices"][0]["message"]["tool_calls"][0]
            .get("extra_content")
            .is_none());
        assert_eq!(cache.get("fc-1").as_deref(), Some("SIG_A"));
    }

    #[test]
    fn cache_skips_strip_without_tool_call_id() {
        let cache = ThoughtSignatureCache::new();
        let mut body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "extra_content": { "google": { "thought_signature": "SIG_A" } },
                        "function": { "name": "f", "arguments": "{}" },
                        "type": "function"
                    }]
                }
            }]
        });
        assert!(!cache_signatures_from_response_value(&mut body, &cache));
        assert!(body["choices"][0]["message"]["tool_calls"][0]["extra_content"]["google"]
            ["thought_signature"]
            .is_string());
    }

    #[test]
    fn cache_message_level_embeds_sigid_and_strips_extra() {
        let cache = ThoughtSignatureCache::new();
        let mut body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello",
                    "extra_content": { "google": { "thought_signature": "SIG_MSG" } }
                }
            }]
        });
        assert!(cache_signatures_from_response_value(&mut body, &cache));
        assert!(body["choices"][0]["message"].get("extra_content").is_none());
        let content = body["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(content.starts_with("Hello\n"), "{}", content);
        assert!(
            content.contains("SIGID:tsig_") && content.contains(THINK_OPEN) && content.contains(THINK_CLOSE),
            "{}",
            content
        );
        let id = content
            .split("SIGID:")
            .nth(1)
            .unwrap()
            .split('<')
            .next()
            .unwrap()
            .trim();
        assert_eq!(cache.get(id).as_deref(), Some("SIG_MSG"));
    }

    #[test]
    fn inject_message_level_from_sigid_inside_think() {
        let cache = ThoughtSignatureCache::new();
        cache.put("tsig_deadbeef", "RESTORED");
        let block = think_wrapped_sigid("tsig_deadbeef");
        let mut body = serde_json::json!({
            "messages": [
                { "role": "user", "content": "Hi" },
                {
                    "role": "assistant",
                    "content": format!("Reply\n{}", block)
                }
            ]
        });
        inject_cached_signatures_into_gemini_request(&mut body, &cache);
        assert_eq!(body["messages"][1]["content"].as_str().unwrap(), "Reply");
        assert_eq!(
            body["messages"][1]["extra_content"]["google"]["thought_signature"]
                .as_str()
                .unwrap(),
            "RESTORED"
        );
    }

    #[test]
    fn inject_message_level_plain_sigid_still_works() {
        let cache = ThoughtSignatureCache::new();
        cache.put("tsig_legacy", "LEG");
        let mut body = serde_json::json!({
            "messages": [{
                "role": "assistant",
                "content": "X\nSIGID:tsig_legacy"
            }]
        });
        inject_cached_signatures_into_gemini_request(&mut body, &cache);
        assert_eq!(body["messages"][0]["content"].as_str().unwrap(), "X");
        assert_eq!(
            body["messages"][0]["extra_content"]["google"]["thought_signature"]
                .as_str()
                .unwrap(),
            "LEG"
        );
    }

    #[test]
    fn inject_request_from_cache() {
        let cache = ThoughtSignatureCache::new();
        cache.put("fc-1", "SIG_X");
        let mut body = serde_json::json!({
            "messages": [
                { "role": "user", "content": "Hi" },
                {
                    "role": "assistant",
                    "tool_calls": [{
                        "function": { "name": "f", "arguments": "{}" },
                        "id": "fc-1",
                        "type": "function"
                    }]
                }
            ]
        });
        inject_cached_signatures_into_gemini_request(&mut body, &cache);
        assert_eq!(
            body["messages"][1]["tool_calls"][0]["extra_content"]["google"]["thought_signature"]
                .as_str()
                .unwrap(),
            "SIG_X"
        );
    }

    #[test]
    fn transform_sse_line_cache_strip_keeps_tool_calls() {
        let cache = ThoughtSignatureCache::new();
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"extra_content":{"google":{"thought_signature":"SIG_B"}},"function":{"name":"f","arguments":"{}"},"id":"fc-1","type":"function"}]}}]}"#;
        let result = transform_sse_line_cache_strip(line, &cache);
        assert!(!result.contains("thought_signature"));
        assert_eq!(cache.get("fc-1").as_deref(), Some("SIG_B"));
        let data = result
            .strip_prefix("data: ")
            .or_else(|| result.strip_prefix("data:"))
            .unwrap()
            .trim();
        let parsed: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(parsed["choices"][0]["delta"]["tool_calls"][0]["function"]["name"], "f");
        assert_eq!(parsed["choices"][0]["delta"]["tool_calls"][0]["id"], "fc-1");
    }

    #[test]
    fn transform_sse_line_message_level_sigid() {
        let cache = ThoughtSignatureCache::new();
        let line = r#"data: {"choices":[{"delta":{"content":"Hi","extra_content":{"google":{"thought_signature":"SSE_MSG"}}}}]}"#;
        let result = transform_sse_line_cache_strip(line, &cache);
        assert!(!result.contains("thought_signature"));
        assert!(!result.contains("SSE_MSG"));
        let data = result
            .strip_prefix("data: ")
            .unwrap()
            .trim();
        let parsed: serde_json::Value = serde_json::from_str(data).unwrap();
        let c = parsed["choices"][0]["delta"]["content"].as_str().unwrap();
        assert!(c.starts_with("Hi\n"), "{}", c);
        assert!(c.contains(THINK_OPEN) && c.contains("SIGID:tsig_"));
        let id = c
            .split("SIGID:")
            .nth(1)
            .unwrap()
            .split('<')
            .next()
            .unwrap()
            .trim();
        assert_eq!(cache.get(id).as_deref(), Some("SSE_MSG"));
    }

    #[test]
    fn transform_sse_chunk_single_trailing_newline() {
        let cache = ThoughtSignatureCache::new();
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"extra_content":{"google":{"thought_signature":"Y"}},"function":{"name":"f","arguments":"{}"},"id":"1","type":"function"}]}}]}"#;
        let chunk = format!("{}\n", line);
        let mut pending = Vec::new();
        let out = transform_sse_chunk_cache_strip(chunk.as_bytes(), &mut pending, &cache);
        let s = String::from_utf8_lossy(&out);
        assert!(!s.ends_with("\n\n"), "{}", s);
        assert_eq!(
            s.as_bytes().iter().filter(|&&b| b == b'\n').count(),
            1
        );
    }
}
