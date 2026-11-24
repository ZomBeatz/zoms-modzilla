//! Optional AI help for troubleshooting: you bring an AI service (Claude, OpenAI, Gemini,
//! OpenRouter, Groq, Mistral, or one on your own computer like Ollama or LM Studio), and the app
//! hands it what it knows about a game and profile (the check, what the game's logs said, the
//! last run), with your home folder and names taken out. Off until you set it up; nothing is sent
//! until you press Send, and you see exactly what goes with it.
//!
//! The key is kept in its own file only you can read (`ai-key`), never in settings, never shown
//! to the page, never logged.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    /// "anthropic", or "openai" for any OpenAI-style service (with `base_url`).
    pub provider: String,
    pub model: String,
    /// For OpenAI-style services: "https://api.openai.com/v1", "http://localhost:11434/v1"…
    #[serde(default)]
    pub base_url: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        AiSettings { provider: "anthropic".into(), model: "claude-opus-5-5".into(), base_url: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// "user" or "assistant".
    pub role: String,
    pub content: String,
}

pub fn load_key(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join("ai-key")).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

pub fn save_key(dir: &Path, key: Option<&str>) -> Result<(), String> {
    let p = dir.join("ai-key");
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        None => {
            let _ = fs::remove_file(&p);
            Ok(())
        }
        Some(k) => {
            use std::os::unix::fs::OpenOptionsExt;
            use std::io::Write;
            let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&p).map_err(|e| e.to_string())?;
            f.write_all(k.as_bytes()).map_err(|e| e.to_string())
        }
    }
}

const SYSTEM: &str = "You help someone use Zom's Modzilla, a mod manager for Linux (Steam, Proton, Steam Deck), to get modded games working. \
Below is what the app knows about their setup, gathered automatically: the game, the profile's mods in load order, the app's own conflict check, \
what the game's mod loaders logged, and how the last run went. Use it before asking questions. \
Answer plainly and briefly, for a player rather than a programmer: name the mod that's most likely at fault and why, then the steps to fix it, \
using the app's own buttons where they exist (Switch off, Update, Find the problem mod, Play without mods, Mod settings, Check again). \
Say when you're not sure, and never invent mod names, versions or settings that aren't in the data. Linux and Proton specifics matter: \
launch options such as WINEDLLOVERRIDES, GE-Proton, case-sensitive file names, Steam Cloud. \
The app also has, per game on the Games page, Game tools: \"Steam's Play starts SKSE\" (script extenders from Game Mode), Windows parts in the prefix \
(Visual C++, .NET, DirectX through Protontricks), modding tools run in the prefix (xEdit, BodySlide, Nemesis, DynDOLOD), and loaders (UE4SS, ReShade). \
Its Help page has guides you can point to by name: Getting started on a Steam Deck; Controller, back buttons and the trackpads; Typing in Game Mode; \
My mods don't show up in the game; SKSE, F4SE and other script extenders from Game Mode; A mod or tool needs Visual C++, .NET or DirectX; \
Steam Cloud keeps undoing my saves; Mods and the SD card; Battery and sleep. \
Never ask for passwords or API keys. \
When the app can do what you suggest, end your answer with one block like this, listing only those actions, and only mods from the list above: \
```actions\n[{\"do\": \"switchOff\", \"mod\": \"<mod name exactly as listed>\"}]\n``` \
The actions are: switchOff and switchOn (a mod, by its name), findProblem (switch half the mods off at a time to find the one at fault), playWithoutMods, checkAgain (the app's conflict check), modSettings (its settings editor). \
The person sees each as a button and decides; leave the block out when no action fits.";

/// Models that take Claude's server-side refusal fallback.
fn takes_fallback(model: &str) -> bool {
    matches!(model, "claude-fable-5-2" | "claude-fable-5-1" | "claude-opus-5-5" | "claude-opus-5" | "claude-sonnet-5-5")
}

/// The request body for Claude's Messages API.
pub fn anthropic_body(model: &str, context: &str, messages: &[Message]) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 16000,
        "system": [
            { "type": "text", "text": SYSTEM },
            // The context changes rarely within a chat: cached, so each reply costs less.
            { "type": "text", "text": format!("What the app knows:\n\n{context}"), "cache_control": { "type": "ephemeral" } }
        ],
        "messages": messages.iter().map(|m| serde_json::json!({ "role": m.role, "content": m.content })).collect::<Vec<_>>(),
    });
    if takes_fallback(model) {
        body["fallbacks"] = serde_json::json!("default");
    }
    body
}

/// The request body for an OpenAI-style chat API.
pub fn openai_body(model: &str, context: &str, messages: &[Message]) -> serde_json::Value {
    let mut all = vec![serde_json::json!({ "role": "system", "content": format!("{SYSTEM}\n\nWhat the app knows:\n\n{context}") })];
    all.extend(messages.iter().map(|m| serde_json::json!({ "role": m.role, "content": m.content })));
    serde_json::json!({ "model": model, "messages": all })
}

/// The reply's text from Claude's answer, or why there isn't one.
pub fn anthropic_reply(v: &serde_json::Value) -> Result<String, String> {
    if v["stop_reason"] == "refusal" {
        let why = v["stop_details"]["explanation"].as_str().unwrap_or("");
        return Err(format!("The AI declined to answer that.{}", if why.is_empty() { String::new() } else { format!(" ({why})") }));
    }
    let text: String = v["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("");
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(if v["stop_reason"] == "max_tokens" { "The answer was cut off before it started. Try a shorter question.".into() } else { "The AI sent back an empty answer.".into() });
    }
    Ok(if v["stop_reason"] == "max_tokens" { format!("{text}\n\n(The answer was cut off.)") } else { text })
}

pub fn openai_reply(v: &serde_json::Value) -> Result<String, String> {
    let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err("The AI sent back an empty answer.".into());
    }
    Ok(if v["choices"][0]["finish_reason"] == "length" { format!("{text}\n\n(The answer was cut off.)") } else { text })
}

/// An error answer, in words.
fn explain(provider: &str, status: reqwest::StatusCode, body: &str) -> String {
    let said = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().or_else(|| v["error"].as_str()).or_else(|| v["message"].as_str()).map(String::from))
        .unwrap_or_default();
    let said = crate::log::scrub(&said.chars().take(300).collect::<String>());
    let what = match status.as_u16() {
        401 | 403 => "The AI service didn't accept the key. Check it in Settings › AI help.".to_string(),
        404 => "The AI service doesn't know that model or address. Check them in Settings › AI help.".to_string(),
        429 => "The AI service says too many requests (or the account is out of credit). Wait a moment and try again.".to_string(),
        400 => "The AI service didn't accept the request.".to_string(),
        500..=599 => "The AI service had a problem. Try again in a moment.".to_string(),
        _ => format!("The AI service answered {status}."),
    };
    let _ = provider;
    if said.is_empty() { what } else { format!("{what} It said: {said}") }
}

/// Is this address one the app may send a key to? Only https, or plain http to this computer.
pub fn allowed_url(base: &str) -> bool {
    let b = base.trim().to_lowercase();
    b.starts_with("https://") || b.starts_with("http://localhost") || b.starts_with("http://127.0.0.1") || b.starts_with("http://[::1]")
}

/// Ask the AI. `context` is the text the person saw and approved.
/// One piece of a streamed answer.
#[derive(Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Stop(String),
    Failed(String),
    Nothing,
}

/// A `data:` line of a streamed answer, read.
pub fn piece(provider: &str, data: &str) -> Piece {
    let data = data.trim();
    if data == "[DONE]" {
        return Piece::Stop("end".into());
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else { return Piece::Nothing };
    if provider == "anthropic" {
        match v["type"].as_str() {
            Some("content_block_delta") if v["delta"]["type"] == "text_delta" => Piece::Text(v["delta"]["text"].as_str().unwrap_or("").to_string()),
            Some("message_delta") => match v["delta"]["stop_reason"].as_str() {
                Some(r) => Piece::Stop(r.to_string()),
                None => Piece::Nothing,
            },
            Some("error") => Piece::Failed(crate::log::scrub(v["error"]["message"].as_str().unwrap_or("The AI service had a problem."))),
            _ => Piece::Nothing,
        }
    } else {
        if let Some(e) = v["error"]["message"].as_str() {
            return Piece::Failed(crate::log::scrub(e));
        }
        let c = &v["choices"][0];
        if let Some(t) = c["delta"]["content"].as_str().filter(|t| !t.is_empty()) {
            return Piece::Text(t.to_string());
        }
        match c["finish_reason"].as_str() {
            Some(r) => Piece::Stop(r.to_string()),
            None => Piece::Nothing,
        }
    }
}

/// Ask the AI. `context` is the text the person saw and approved. The answer streams: each
/// piece goes to `on_text` as it comes, and the whole answer is given back at the end.
pub async fn ask(http: &reqwest::Client, s: &AiSettings, key: Option<&str>, context: &str, messages: &[Message], on_text: impl Fn(&str)) -> Result<String, String> {
    if messages.is_empty() || messages.last().is_some_and(|m| m.role != "user") {
        return Err("Write a question first.".into());
    }
    if s.model.trim().is_empty() {
        return Err("Pick a model in Settings › AI help.".into());
    }
    let timeout = Duration::from_secs(600);
    let res = if s.provider == "anthropic" {
        let key = key.ok_or("Add your Claude API key in Settings › AI help.")?;
        let mut req = http
            .post("https://api.anthropic.com/v1/messages")
            .timeout(timeout)
            .header("x-api-key", key)
            .header("anthropic-version", "2026-10-21")
            .header("content-type", "application/json");
        if takes_fallback(&s.model) {
            req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        let mut body = anthropic_body(&s.model, context, messages);
        body["stream"] = serde_json::json!(true);
        req.json(&body).send().await
    } else {
        let base = s.base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            return Err("Add the AI service's address in Settings › AI help.".into());
        }
        if !allowed_url(base) {
            return Err("The AI service's address has to start with https:// (or be on this computer).".into());
        }
        let mut req = http.post(format!("{base}/chat/completions")).timeout(timeout).header("content-type", "application/json");
        if let Some(k) = key {
            req = req.bearer_auth(k);
        }
        let mut body = openai_body(&s.model, context, messages);
        body["stream"] = serde_json::json!(true);
        req.json(&body).send().await
    }
    .map_err(|e| if e.is_timeout() { "The AI service took too long to answer.".to_string() } else if e.is_connect() { "Couldn't reach the AI service. For one on this computer, check it's running.".to_string() } else { "Couldn't reach the AI service.".to_string() })?;
    let status = res.status();
    let streamed = res.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("event-stream"));
    if !status.is_success() || !streamed {
        // An error, or a service that answered in one piece anyway.
        let body = res.text().await.map_err(|_| "The AI service's answer didn't arrive whole.".to_string())?;
        if !status.is_success() {
            return Err(explain(&s.provider, status, &body));
        }
        let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| "The AI service's answer didn't read.".to_string())?;
        let text = if s.provider == "anthropic" { anthropic_reply(&v) } else { openai_reply(&v) }?;
        on_text(&text);
        return Ok(text);
    }
    let mut res = res;
    let mut buf = String::new();
    let mut all = String::new();
    let mut stop = String::new();
    while let Some(chunk) = res.chunk().await.map_err(|_| "The AI service stopped answering partway.".to_string())? {
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(i) = buf.find('\n') {
            let line: String = buf.drain(..=i).collect();
            let Some(data) = line.trim_end().strip_prefix("data:") else { continue };
            match piece(&s.provider, data) {
                Piece::Text(t) => {
                    all.push_str(&t);
                    on_text(&t);
                }
                Piece::Stop(r) => stop = r,
                Piece::Failed(e) => return Err(format!("The AI service had a problem partway: {e}")),
                Piece::Nothing => {}
            }
        }
        if all.len() > 400_000 {
            break;
        }
    }
    if stop == "refusal" {
        return Err("The AI declined to answer that.".into());
    }
    let all = all.trim().to_string();
    if all.is_empty() {
        return Err("The AI sent back an empty answer.".into());
    }
    Ok(if matches!(stop.as_str(), "max_tokens" | "length") { format!("{all}\n\n(The answer was cut off.)") } else { all })
}

/// "2026-11-23 01:15" in this computer's time zone.
pub fn date(t: u64) -> String {
    let t = crate::log::local(t);
    let days = (t / 86_400) as i64;
    let secs = t % 86_400;
    // Days since 1970 to a calendar date (Howard Hinnant's method).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

/// Personal details out of text going to an AI service: the home folder, user names, keys.
pub fn scrub(text: &str, names: &[String]) -> String {
    let mut t = text.to_string();
    if let Some(home) = std::env::var_os("HOME").map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 1) {
        t = t.replace(&home, "~");
    }
    for n in names.iter().filter(|n| n.len() >= 3) {
        t = t.replace(n.as_str(), "<you>");
    }
    t.lines().map(crate::log::scrub).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_requests_and_reads_answers() {
        let m = vec![Message { role: "user".into(), content: "Why does it crash?".into() }];
        let b = anthropic_body("claude-opus-5-5", "Game: Stardew", &m);
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["fallbacks"], "default");
        assert!(b.get("temperature").is_none());
        assert!(b["system"][1]["text"].as_str().unwrap().contains("Game: Stardew"));
        assert!(anthropic_body("claude-haiku-5-5", "", &m).get("fallbacks").is_none());
        let o = openai_body("llama3", "ctx", &m);
        assert_eq!(o["messages"][0]["role"], "system");
        assert_eq!(o["messages"][1]["content"], "Why does it crash?");

        let ok = serde_json::json!({ "content": [{ "type": "thinking", "thinking": "" }, { "type": "text", "text": "Switch off X." }], "stop_reason": "end_turn" });
        assert_eq!(anthropic_reply(&ok).unwrap(), "Switch off X.");
        let no = serde_json::json!({ "content": [], "stop_reason": "refusal", "stop_details": { "explanation": "nope" } });
        assert!(anthropic_reply(&no).unwrap_err().contains("declined"));
        let oa = serde_json::json!({ "choices": [{ "message": { "content": "Do Y." }, "finish_reason": "stop" }] });
        assert_eq!(openai_reply(&oa).unwrap(), "Do Y.");
        assert!(allowed_url("https://api.openai.com/v1") && allowed_url("http://localhost:11434/v1") && !allowed_url("http://example.com/v1"));
        let s = scrub("key=abcdef&x by Alex", &["Alex".into()]);
        assert!(s.contains("key=…") && s.contains("<you>") && !s.contains("abcdef"));
    }

    #[test]
    fn reads_streamed_answers() {
        assert_eq!(piece("anthropic", r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hi"}}"#), Piece::Text("Hi".into()));
        assert_eq!(piece("anthropic", r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}"#), Piece::Nothing);
        assert_eq!(piece("anthropic", r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#), Piece::Stop("end_turn".into()));
        assert!(matches!(piece("anthropic", r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#), Piece::Failed(_)));
        assert_eq!(piece("openai", r#"{"choices":[{"delta":{"content":"Yo"},"finish_reason":null}]}"#), Piece::Text("Yo".into()));
        assert_eq!(piece("openai", "[DONE]"), Piece::Stop("end".into()));
        assert_eq!(piece("openai", "not json"), Piece::Nothing);
    }

    #[test]
    fn keeps_the_key_private() {
        let d = tempfile::tempdir().unwrap();
        save_key(d.path(), Some(" sk-test ")).unwrap();
        assert_eq!(load_key(d.path()).as_deref(), Some("sk-test"));
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(d.path().join("ai-key")).unwrap().permissions().mode() & 0o777, 0o600);
        save_key(d.path(), None).unwrap();
        assert!(load_key(d.path()).is_none());
    }
}
