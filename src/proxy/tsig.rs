//! Gemini thought-signature (TSIG) encoding/decoding for multi-step function
//! calling across OpenAI-compatible clients.
//!
//! Gemini 3 models return `thought_signature` in their OpenAI-compatible
//! responses (inside `extra_content.google.thought_signature` on tool_calls
//! or message parts).  These must be echoed back in subsequent requests for
//! multi-step function calling validation.  Since most OpenAI SDKs silently
//! drop unknown fields, we encode each signature as a `<think>TSIG:…</think>`
//! marker prepended to assistant `content`. Clients naturally preserve content in
//! conversation history, so the marker survives round-trips.
//!
//! **Response path (encode):** extract signatures from upstream Gemini
//! responses and prepend `<think>TSIG:sig</think>` to `choices[].message.content`
//! (non-streaming) or inject a synthetic content delta (streaming).
//!
//! **Request path (decode):** strip `<think>TSIG:…</think>` markers from assistant
//! content.  When routing to a Gemini backend, write them back to
//! `tool_calls[0].extra_content.google.thought_signature`.

use bytes::Bytes;

const TSIG_PREFIX: &str = "<think>TSIG:";
const TSIG_SUFFIX: &str = "</think>";

fn encode_tsig(sig: &str) -> String {
    format!("{}{}{}", TSIG_PREFIX, sig, TSIG_SUFFIX)
}

/// Extract all `<think>TSIG:…</think>` markers from `content`, returning the cleaned
/// string and the list of extracted signature values (in order).
fn strip_tsig_markers(content: &str) -> (String, Vec<String>) {
    let mut sigs = Vec::new();
    let mut cleaned = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find(TSIG_PREFIX) {
        cleaned.push_str(&rest[..start]);
        let after = &rest[start + TSIG_PREFIX.len()..];
        if let Some(end) = after.find(TSIG_SUFFIX) {
            sigs.push(after[..end].to_string());
            rest = &after[end + TSIG_SUFFIX.len()..];
        } else {
            cleaned.push_str(TSIG_PREFIX);
            rest = after;
        }
    }
    cleaned.push_str(rest);
    (cleaned, sigs)
}

/// Extract `thought_signature` from `obj["extra_content"]["google"]`, remove
/// the field, and clean up empty parent objects.
fn extract_and_remove_sig(obj: &mut serde_json::Value) -> Option<String> {
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

    Some(sig)
}

// --------------- response path (encode) ---------------

/// For a non-streaming response body, extract all thought_signatures from
/// tool_calls and message-level extra_content, encode them as `<think>TSIG:…</think>`
/// markers prepended to `choices[].message.content`. Returns `Some(modified)`
/// if any signatures were found, `None` otherwise (caller keeps original).
pub fn inject_tsig_response_body(body: &[u8]) -> Option<Bytes> {
    let mut val: serde_json::Value = serde_json::from_slice(body).ok()?;
    let mut modified = false;

    let choices = val.get_mut("choices")?.as_array_mut()?;
    for choice in choices.iter_mut() {
        let message = match choice.get_mut("message") {
            Some(m) => m,
            None => continue,
        };
        let mut sigs: Vec<String> = Vec::new();

        if let Some(tcs) = message
            .get_mut("tool_calls")
            .and_then(|v| v.as_array_mut())
        {
            for tc in tcs.iter_mut() {
                if let Some(s) = extract_and_remove_sig(tc) {
                    sigs.push(s);
                }
            }
        }

        if let Some(s) = extract_and_remove_sig(message) {
            sigs.push(s);
        }

        if !sigs.is_empty() {
            let marker: String = sigs.iter().map(|s| encode_tsig(s)).collect();
            let existing = message
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            message["content"] =
                serde_json::Value::String(format!("{}{}", marker, existing));
            modified = true;
        }
    }

    if modified {
        serde_json::to_vec(&val).ok().map(Bytes::from)
    } else {
        None
    }
}

/// Transform a single SSE `data: {…}` line: extract thought_signatures from
/// delta tool_calls / delta-level extra_content and inject `<think>TSIG:…</think>`
/// into `delta.content` (prepended). Non-data lines and `[DONE]` are returned unchanged.
fn transform_sse_data_line(line: &str) -> String {
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

    let mut found_any = false;

    if let Some(choices) = chunk.get_mut("choices").and_then(|v| v.as_array_mut()) {
        for choice in choices.iter_mut() {
            let delta = match choice.get_mut("delta") {
                Some(d) => d,
                None => continue,
            };
            let mut sigs: Vec<String> = Vec::new();

            if let Some(tcs) = delta
                .get_mut("tool_calls")
                .and_then(|v| v.as_array_mut())
            {
                for tc in tcs.iter_mut() {
                    if let Some(s) = extract_and_remove_sig(tc) {
                        sigs.push(s);
                    }
                }
            }

            if let Some(s) = extract_and_remove_sig(delta) {
                sigs.push(s);
            }

            if !sigs.is_empty() {
                let marker: String = sigs.iter().map(|s| encode_tsig(s)).collect();
                let existing = delta
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                delta["content"] =
                    serde_json::Value::String(format!("{}{}", marker, existing));
                found_any = true;
            }
        }
    }

    if found_any {
        format!(
            "data: {}",
            serde_json::to_string(&chunk).unwrap_or_else(|_| data.to_string())
        )
    } else {
        line.to_string()
    }
}

/// Process a raw SSE byte chunk: buffer incomplete lines in `pending`,
/// transform complete `data:` lines that contain thought_signatures.
pub fn transform_sse_chunk(chunk: &[u8], pending: &mut Vec<u8>) -> Bytes {
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
    for line in complete_str.split('\n') {
        if !line.is_empty() {
            output.push_str(&transform_sse_data_line(line));
        }
        output.push('\n');
    }
    Bytes::from(output.into_bytes())
}

// --------------- request path (decode) ---------------

/// Result of stripping TSIG markers from a request body.
pub struct TsigStripResult {
    pub body: serde_json::Value,
    pub sigs: Vec<(usize, Vec<String>)>,
}

/// Strip `<think>TSIG:…</think>` markers from assistant messages in a chat completions
/// request body.  Returns `None` if no markers were found.
pub fn strip_tsig_from_request(body: &[u8]) -> Option<TsigStripResult> {
    let mut val: serde_json::Value = serde_json::from_slice(body).ok()?;
    let messages = val.get_mut("messages")?.as_array_mut()?;
    let mut all_sigs: Vec<(usize, Vec<String>)> = Vec::new();

    for (i, msg) in messages.iter_mut().enumerate() {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "assistant" {
            continue;
        }
        let content = match msg.get("content").and_then(|v| v.as_str()) {
            Some(c) if c.contains(TSIG_PREFIX) => c.to_string(),
            _ => continue,
        };
        let (cleaned, extracted) = strip_tsig_markers(&content);
        if !extracted.is_empty() {
            msg["content"] = serde_json::Value::String(cleaned);
            all_sigs.push((i, extracted));
        }
    }

    if all_sigs.is_empty() {
        None
    } else {
        Some(TsigStripResult {
            body: val,
            sigs: all_sigs,
        })
    }
}

/// Write extracted TSIG signatures back into the request JSON for forwarding
/// to a Gemini backend.  Places the first signature from each message at
/// `messages[idx].tool_calls[0].extra_content.google.thought_signature`.
/// If the message has no tool_calls, the signature is placed at message level.
pub fn inject_tsig_to_gemini_request(
    val: &mut serde_json::Value,
    sigs: &[(usize, Vec<String>)],
) {
    let messages = match val.get_mut("messages").and_then(|v| v.as_array_mut()) {
        Some(m) => m,
        None => return,
    };
    for (msg_idx, sig_list) in sigs {
        let sig = match sig_list.first() {
            Some(s) => s,
            None => continue,
        };
        let msg = match messages.get_mut(*msg_idx) {
            Some(m) => m,
            None => continue,
        };
        let has_tool_calls = msg
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .map_or(false, |a| !a.is_empty());

        if has_tool_calls {
            msg["tool_calls"][0]["extra_content"]["google"]["thought_signature"] =
                serde_json::Value::String(sig.clone());
        } else {
            msg["extra_content"]["google"]["thought_signature"] =
                serde_json::Value::String(sig.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let sig = "abc123==";
        let encoded = encode_tsig(sig);
        assert_eq!(encoded, "<think>TSIG:abc123==</think>");
        let (cleaned, sigs) = strip_tsig_markers(&encoded);
        assert_eq!(cleaned, "");
        assert_eq!(sigs, vec!["abc123=="]);
    }

    #[test]
    fn strip_preserves_surrounding_text() {
        let content = "Hello<think>TSIG:sig1</think>world<think>TSIG:sig2</think>!";
        let (cleaned, sigs) = strip_tsig_markers(content);
        assert_eq!(cleaned, "Helloworld!");
        assert_eq!(sigs, vec!["sig1", "sig2"]);
    }

    #[test]
    fn strip_no_markers() {
        let (cleaned, sigs) = strip_tsig_markers("Hello world");
        assert_eq!(cleaned, "Hello world");
        assert!(sigs.is_empty());
    }

    #[test]
    fn strip_unclosed_marker() {
        let (cleaned, sigs) = strip_tsig_markers("Hello<think>TSIG:oops");
        assert_eq!(cleaned, "Hello<think>TSIG:oops");
        assert!(sigs.is_empty());
    }

    #[test]
    fn inject_response_body_tool_call_sig() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "extra_content": { "google": { "thought_signature": "SIG_A" } },
                        "function": { "name": "f", "arguments": "{}" },
                        "id": "fc-1", "type": "function"
                    }]
                }
            }]
        });
        let result =
            inject_tsig_response_body(&serde_json::to_vec(&body).unwrap()).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        let content = parsed["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(content.starts_with("<think>TSIG:SIG_A</think>"));
        assert!(parsed["choices"][0]["message"]["tool_calls"][0]
            .get("extra_content")
            .is_none());
    }

    #[test]
    fn inject_response_body_message_level_sig() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "thinking…",
                    "extra_content": { "google": { "thought_signature": "SIG_C" } }
                }
            }]
        });
        let result =
            inject_tsig_response_body(&serde_json::to_vec(&body).unwrap()).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        let content = parsed["choices"][0]["message"]["content"].as_str().unwrap();
        assert_eq!(content, "<think>TSIG:SIG_C</think>thinking…");
        assert!(parsed["choices"][0]["message"]
            .get("extra_content")
            .is_none());
    }

    #[test]
    fn inject_response_body_no_sig_returns_none() {
        let body = serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "Hello" } }]
        });
        assert!(
            inject_tsig_response_body(&serde_json::to_vec(&body).unwrap()).is_none()
        );
    }

    #[test]
    fn strip_request_extracts_sigs() {
        let body = serde_json::json!({
            "model": "gemini-3",
            "messages": [
                { "role": "user", "content": "Hi" },
                {
                    "role": "assistant",
                    "content": "<think>TSIG:SIG_A</think>",
                    "tool_calls": [{
                        "function": { "name": "f", "arguments": "{}" },
                        "id": "fc-1", "type": "function"
                    }]
                },
                { "role": "tool", "content": "{}", "tool_call_id": "fc-1" }
            ]
        });
        let r = strip_tsig_from_request(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(r.sigs.len(), 1);
        assert_eq!(r.sigs[0].0, 1);
        assert_eq!(r.sigs[0].1, vec!["SIG_A"]);
        assert_eq!(r.body["messages"][1]["content"].as_str().unwrap(), "");
    }

    #[test]
    fn inject_to_gemini_request_writes_back() {
        let mut body = serde_json::json!({
            "messages": [
                { "role": "user", "content": "Hi" },
                {
                    "role": "assistant", "content": "",
                    "tool_calls": [{
                        "function": { "name": "f", "arguments": "{}" },
                        "id": "fc-1", "type": "function"
                    }]
                }
            ]
        });
        inject_tsig_to_gemini_request(&mut body, &[(1, vec!["SIG_A".into()])]);
        assert_eq!(
            body["messages"][1]["tool_calls"][0]["extra_content"]["google"]
                ["thought_signature"]
                .as_str()
                .unwrap(),
            "SIG_A"
        );
    }

    #[test]
    fn transform_sse_line_with_sig() {
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"extra_content":{"google":{"thought_signature":"SIG_B"}},"function":{"name":"f","arguments":"{}"},"id":"fc-1","type":"function"}]}}]}"#;
        let result = transform_sse_data_line(line);
        assert!(result.contains("<think>TSIG:SIG_B</think>"));
        assert!(!result.contains("thought_signature"));
    }

    #[test]
    fn transform_sse_line_no_sig_unchanged() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        assert_eq!(transform_sse_data_line(line), line);
    }

    #[test]
    fn transform_sse_done_unchanged() {
        assert_eq!(transform_sse_data_line("data: [DONE]"), "data: [DONE]");
    }

    #[test]
    fn transform_sse_chunk_basic() {
        let mut pending = Vec::new();
        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n";
        let out = transform_sse_chunk(chunk, &mut pending);
        assert!(pending.is_empty());
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("\"hi\""));
    }

    #[test]
    fn transform_sse_chunk_with_sig() {
        let mut pending = Vec::new();
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"extra_content":{"google":{"thought_signature":"X"}},"function":{"name":"f","arguments":"{}"},"id":"1","type":"function"}]}}]}"#;
        let chunk = format!("{}\n\n", line);
        let out = transform_sse_chunk(chunk.as_bytes(), &mut pending);
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("<think>TSIG:X</think>"), "output: {}", s);
        assert!(!s.contains("thought_signature"), "output: {}", s);
    }

    #[test]
    fn transform_sse_chunk_partial_line_buffered() {
        let mut pending = Vec::new();
        let part1 = b"data: {\"cho";
        let out1 = transform_sse_chunk(part1, &mut pending);
        assert!(out1.is_empty());
        assert!(!pending.is_empty());

        let part2 = b"ices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n";
        let out2 = transform_sse_chunk(part2, &mut pending);
        assert!(pending.is_empty());
        let s = String::from_utf8_lossy(&out2);
        assert!(s.contains("\"ok\""));
    }
}
