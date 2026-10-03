// Claude API client — the same integration as ClaudeService.swift: multi-turn
// chat with web search, and files sent as document/image/text blocks.
//
// Everything happens here rather than in the island: the API key never leaves
// the Secret Service keyring, and file bytes never cross the IPC boundary.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::secrets;

const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Is this base a relay on this machine? `http://localhost:20128/v1` is a proxy
/// the user runs; `http://relay.example.com` is somebody else's.
fn is_loopback(base: &str) -> bool {
    let rest = match base.split_once("://") {
        Some((_, r)) => r,
        None => return false, // no scheme: not something we will hand a key to
    };
    // An IPv6 literal keeps its brackets (`[::1]:8642`), and splitting on ':'
    // alone would leave just `[` — so trim that shape first.
    let authority = rest.split('/').next().unwrap_or("");
    let host = authority
        .strip_prefix('[')
        .and_then(|h| h.split(']').next())
        .unwrap_or_else(|| authority.split(':').next().unwrap_or(""))
        .to_ascii_lowercase();
    matches!(host.as_str(), "localhost" | "::1")
        || host == "127.0.0.1"
        || (host.starts_with("127.") && host.split('.').count() == 4)
        || host == "0.0.0.0"
}

/// The Messages endpoint to call: the configured base URL with /v1/messages
/// appended, unless the base already ends in /v1/messages. Same-body-compatible
/// relays (Chinese model relays, LiteLLM, corporate gateways) then work with a
/// single field. Empty/None → the official API.
pub fn endpoint_for(api_base: Option<&str>) -> String {
    let base = match api_base.map(str::trim) {
        Some(b) if !b.is_empty() => b.trim_end_matches('/'),
        _ => return DEFAULT_ENDPOINT.to_string(),
    };
    let lower = base.to_lowercase();
    if lower.ends_with("/messages") {
        // Already a full endpoint path — take it verbatim.
        base.to_string()
    } else if lower.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    }
}
/// Server-side fallback: on a policy decline the API retries the same request on
/// a fallback model inside the same call, so the island never shows a dead end.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const MAX_TOKENS: u32 = 4096;
/// Text and code files are inlined; anything larger is skipped, as on macOS.
const MAX_INLINE_TEXT: u64 = 200_000;

pub const DEFAULT_MODEL: &str = "claude-opus-5";

const SYSTEM_PROMPT: &str = "You are Mochi, a personal AI assistant living at the top of the user's screen. \
You have web search access and can help with absolutely anything — research, coding, finding places, recommendations, tasks, questions. \
Answer in the same language the user writes in. Be thorough and complete — use as much detail as the task requires. \
No markdown formatting (no **, no ##, no bullet dashes). Use plain text with line breaks.";

#[derive(Default)]
pub struct Chat {
    /// Full multi-turn history, including tool_use / tool_result blocks.
    messages: Mutex<Vec<Value>>,
}

impl Chat {
    pub fn reset(&self) {
        self.messages.lock().unwrap().clear();
    }

    fn is_empty(&self) -> bool {
        self.messages.lock().unwrap().is_empty()
    }

    fn push(&self, message: Value) {
        self.messages.lock().unwrap().push(message);
    }

    fn pop(&self) {
        self.messages.lock().unwrap().pop();
    }

    fn snapshot(&self) -> Vec<Value> {
        self.messages.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatContext {
    File { name: String, path: String },
    Window { app_name: String, title: String, url: Option<String> },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatReply {
    pub text: String,
}

/// One chat turn. Returns the assistant's text, or a message the island shows
/// in the note view.
pub async fn send(
    chat: &Chat,
    model: &str,
    api_base: Option<&str>,
    query: String,
    context: Option<ChatContext>,
) -> Result<ChatReply, String> {
    // Where may the key go? The official API, and a relay on loopback — that is a
    // proxy on this same machine (9router and friends), and 9router rejects a
    // Messages request without one. Anywhere else: no. `api_base` is a plain
    // string in a 0644 JSON file, so any same-uid process can rewrite it and read
    // the user's Anthropic key out of the next chat message — a persistence trick
    // that works even where the keyring collection itself is locked.
    let custom = api_base.map(str::trim).filter(|b| !b.is_empty());
    let key = match custom {
        Some(base) if !is_loopback(base) => None,
        _ => match secrets::get("anthropic-api-key") {
            Some(k) => Some(k),
            None if custom.is_some() => None,
            None => return Err("API key missing. Open settings.".to_string()),
        },
    };

    let mut content: Vec<Value> = Vec::new();

    // File / window context rides along with the first message only, exactly
    // like ClaudeService.chat().
    if chat.is_empty() {
        match &context {
            Some(ChatContext::File { name, path }) => {
                if let Some(block) = file_block(path) {
                    content.push(block);
                }
                content.push(json!({ "type": "text", "text": format!("File: {name}") }));
            }
            Some(ChatContext::Window { app_name, title, url }) => {
                let mut text = format!("Context — App: {app_name}, Window: {title}");
                if let Some(url) = url {
                    text.push_str(&format!(", URL: {url}"));
                }
                content.push(json!({ "type": "text", "text": text }));
            }
            None => {}
        }
    }
    content.push(json!({ "type": "text", "text": query }));

    chat.push(json!({ "role": "user", "content": content }));

    let body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": SYSTEM_PROMPT,
        "tools": [{ "type": "web_search_20260209", "name": "web_search", "max_uses": 5 }],
        "fallbacks": "default",
        "messages": chat.snapshot(),
    });

    let response = match call(key.as_deref(), api_base, &body).await {
        Ok(v) => v,
        Err(err) => {
            chat.pop(); // keep the history consistent with what the model saw
            return Err(err);
        }
    };

    // A policy decline comes back as HTTP 200 with stop_reason "refusal".
    if response.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        chat.pop();
        let why = response
            .get("stop_details")
            .and_then(|d| d.get("explanation"))
            .and_then(Value::as_str)
            .unwrap_or("Claude declined this one.");
        return Err(why.to_string());
    }

    let Some(blocks) = response.get("content").and_then(Value::as_array).cloned() else {
        chat.pop();
        return Err(describe(&response));
    };

    // Store the whole content — tool_use / tool_result blocks included — so the
    // next turn has the right context.
    chat.push(json!({ "role": "assistant", "content": blocks.clone() }));

    let text = blocks
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() {
        return Err("No response text.".into());
    }
    Ok(ChatReply { text })
}

/// What a 200 that is not a Messages response actually was. "Unexpected API
/// response." was technically true and useless: an OpenAI-shaped relay, a proxy
/// that wraps an error in a 200 and a gateway returning HTML all land on that one
/// sentence, and what the user can act on — what came back — is exactly what it
/// refuses to show. Nothing secret is in here: the body, truncated.
fn describe(response: &Value) -> String {
    if let Some(message) = response
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
    {
        return format!("Relay returned an error: {message}");
    }
    // An OpenAI-shaped relay: choices[0].message.content
    if let Some(text) = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
    {
        return format!("This relay answers in OpenAI's format, not Anthropic's: {text}");
    }
    let fields: Vec<&str> =
        response.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
    let listed = if fields.is_empty() { "nothing".to_string() } else { fields.join(", ") };
    let preview: String = response.to_string().chars().take(160).collect();
    format!("Relay answered 200 with no messages content (fields: {listed}). {preview}")
}

async fn call(key: Option<&str>, api_base: Option<&str>, body: &Value) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        // reqwest strips `Authorization` on a cross-host redirect but leaves
        // `x-api-key` alone, so a relay answering 302 to anywhere else would
        // receive the credential verbatim. A relay that needs to move the user
        // somewhere should say so with a real URL.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    let url = endpoint_for(api_base);
    let mut request = client
        .post(&url)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(body);
    // The server-side-fallback beta is an Anthropic-API feature; a relay that
    // merely speaks the same wire format may reject unknown beta flags.
    if api_base.map(str::trim).filter(|b| !b.is_empty()).is_none() {
        request = request.header("anthropic-beta", FALLBACK_BETA);
    }
    if let Some(key) = key {
        request = request.header("x-api-key", key);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        // Surface the API's own message, which is what makes a bad key obvious.
        let detail = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(format!("Claude API {status}: {detail}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("Bad API response: {e}"))
}

/// PDF → document block, image → image block, text/code → inline text.
/// Mirrors readFileAsBlock() in ClaudeService.swift.
fn file_block(path: &str) -> Option<Value> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let media_type = match ext.as_str() {
        "pdf" => Some(("document", "application/pdf")),
        "jpg" | "jpeg" => Some(("image", "image/jpeg")),
        "png" => Some(("image", "image/png")),
        "gif" => Some(("image", "image/gif")),
        "webp" => Some(("image", "image/webp")),
        _ => None,
    };

    if let Some((block_type, media)) = media_type {
        let bytes = std::fs::read(path).ok()?;
        return Some(json!({
            "type": block_type,
            "source": { "type": "base64", "media_type": media, "data": base64(&bytes) },
        }));
    }

    let len = std::fs::metadata(path).ok()?.len();
    if len > MAX_INLINE_TEXT {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    Some(json!({ "type": "text", "text": format!("File contents:\n{text}") }))
}

/// Small standalone base64 encoder — not worth another dependency.
/// Also used for Stripe's basic auth.
pub(crate) fn base64_for(bytes: &[u8]) -> String {
    base64(bytes)
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{base64, describe, is_loopback};
    use serde_json::json;

    /// A relay that answers 200 with something else used to produce the same
    /// three words for every cause, which is the one thing an error message
    /// must not be. Each shape a real relay actually returns has to name itself.
    #[test]
    fn a_wrong_shaped_200_says_what_actually_came_back() {
        let wrapped = describe(&json!({"error": {"message": "model hermes-aux not found"}}));
        assert!(wrapped.contains("model hermes-aux not found"), "got: {wrapped}");

        let openai = describe(&json!({"choices": [{"message": {"content": "hello"}}]}));
        assert!(openai.contains("OpenAI"), "got: {openai}");

        let html = describe(&json!("<!doctype html><title>proxy</title>"));
        assert!(html.contains("no messages content"), "got: {html}");

        // Nothing here may echo anything credential-shaped.
        for text in [&wrapped, &openai, &html] {
            assert!(!text.contains("sk-"), "a key leaked into an error: {text}");
        }
    }

    /// The key is the one credential this app holds, and `api_base` is a plain
    /// string in a world-readable JSON file — so a relay only ever sees it when
    /// the relay is on this machine.
    #[test]
    fn only_local_relays_may_receive_the_key() {
        assert!(is_loopback("http://localhost:20128/v1"));
        assert!(is_loopback("http://127.0.0.1:8642"));
        assert!(is_loopback("HTTP://127.9.9.9/v1"));
        assert!(is_loopback("http://[::1]:1234/v1"));
        // A host that merely *starts* with a loopback-looking name is not loopback.
        assert!(!is_loopback("http://localhost.attacker.example/v1"));
        assert!(!is_loopback("https://api.anthropic.com"));
        assert!(!is_loopback("http://192.168.1.10:20128/v1"));
        assert!(!is_loopback("20128"));
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    // A relay that speaks the Messages wire format must be reachable by merely
    // pasting its base — every reasonable spelling lands on the same URL.
    #[test]
    fn endpoint_resolves_the_way_a_relay_base_is_spelled() {
        let e = super::endpoint_for;
        assert_eq!(e(None), super::DEFAULT_ENDPOINT);
        assert_eq!(e(Some("")), super::DEFAULT_ENDPOINT);
        assert_eq!(e(Some("  ")), super::DEFAULT_ENDPOINT);
        assert_eq!(e(Some("https://relay.cn")), "https://relay.cn/v1/messages");
        assert_eq!(e(Some("https://relay.cn/")), "https://relay.cn/v1/messages");
        assert_eq!(e(Some("https://relay.cn/v1")), "https://relay.cn/v1/messages");
        assert_eq!(e(Some("https://relay.cn/v1/")), "https://relay.cn/v1/messages");
        assert_eq!(e(Some("https://relay.cn/v1/messages")), "https://relay.cn/v1/messages");
        // A base that names /messages without /v1 is taken verbatim, not guessed at.
        assert_eq!(e(Some("https://gw.lan/anthropic/messages")), "https://gw.lan/anthropic/messages");
    }
}
