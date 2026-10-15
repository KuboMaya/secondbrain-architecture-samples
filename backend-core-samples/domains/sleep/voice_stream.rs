//! Portfolio sample — speech chunking + Fire-and-Forget TTS POST.
//! Highlights: punctuation drain for streaming deltas; style-tag parse; tokio::spawn POST.
//! Source: excerpt from src-tauri/src/domains/sleep/ai_handler.rs

//! Sleep-domain AI post-process: stream chunking + A.I.VOICE style tags.
//! Gemini transport stays in `domains/ai`; this module only parses / maps.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Nested emotion styles for A.I.VOICE 結月ゆかり.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceStyles {
    pub joy: f32,
    pub anger: f32,
    pub sad: f32,
}

impl Default for VoiceStyles {
    fn default() -> Self {
        Self {
            joy: 0.0,
            anger: 0.0,
            sad: 0.0,
        }
    }
}

/// JSON body posted to the C# A.I.VOICE speak server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceSpeakRequest {
    pub text: String,
    pub speed: f32,
    pub pitch: f32,
    pub pitch_range: f32,
    pub styles: VoiceStyles,
}

impl Default for VoiceSpeakRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            speed: 1.0,
            pitch: 1.0,
            pitch_range: 1.0,
            styles: VoiceStyles::default(),
        }
    }
}

/// Metadata tags embedded in Gemini speech text, e.g. `[joy:0.6]`.
static VOICE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[([a-z_]+):([+-]?(?:\d+(?:\.\d+)?|\.\d+))\]").expect("voice tag regex")
});

const SPEECH_PUNCT: &[char] = &['。', '、', '！', '？', '!', '?', '．', '｡'];

/// True if `c` ends a speakable chunk for streaming TTS.
pub fn is_speech_punctuation(c: char) -> bool {
    SPEECH_PUNCT.contains(&c)
}

/// Drain complete punctuated chunks from a streaming buffer; remainder stays in `buffer`.
pub fn drain_speech_chunks(buffer: &mut String) -> Vec<String> {
    let mut chunks = Vec::new();
    loop {
        let end = buffer.char_indices().find_map(|(i, c)| {
            if is_speech_punctuation(c) {
                Some(i + c.len_utf8())
            } else {
                None
            }
        });
        let Some(end) = end else { break };
        let chunk = buffer[..end].to_string();
        buffer.drain(..end);
        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }
    }
    chunks
}

/// Extract style/speed tags into a request and strip every tag from `text`.
/// Unspecified numeric fields keep defaults (speed/pitch/pitch_range=1.0, styles=0.0).
pub fn parse_voice_chunk(raw: &str) -> VoiceSpeakRequest {
    let mut req = VoiceSpeakRequest::default();
    for caps in VOICE_TAG_RE.captures_iter(raw) {
        let key = caps[1].to_ascii_lowercase();
        let Ok(value) = caps[2].parse::<f32>() else {
            continue;
        };
        match key.as_str() {
            "joy" => req.styles.joy = value,
            "anger" => req.styles.anger = value,
            "sad" => req.styles.sad = value,
            "speed" => req.speed = value,
            "pitch" => req.pitch = value,
            "pitch_range" => req.pitch_range = value,
            _ => {}
        }
    }
    let stripped = VOICE_TAG_RE.replace_all(raw, "");
    req.text = collapse_ws(stripped.as_ref());
    req
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Split `text` into punctuated chunks (remainder as final chunk) and map tags.
pub fn chunk_and_parse_speech(text: &str) -> Vec<VoiceSpeakRequest> {
    let mut buffer = text.to_string();
    let mut chunks = drain_speech_chunks(&mut buffer);
    if !buffer.trim().is_empty() {
        chunks.push(std::mem::take(&mut buffer));
    }
    chunks
        .into_iter()
        .map(|c| parse_voice_chunk(&c))
        .filter(|r| !r.text.trim().is_empty())
        .collect()
}

/// Fire-and-forget POST each chunk to `{voice_server_url}/speak` (Keep-Alive client).
pub fn speak_via_voice_server(client: &reqwest::Client, voice_server_url: &str, text: &str) {
    let base = voice_server_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return;
    }
    let url = format!("{base}/speak");
    for req in chunk_and_parse_speech(text) {
        post_speak_request(client, &url, req);
    }
}

/// POST a single already-parsed speak request (stream path).
pub fn speak_parsed_request(client: &reqwest::Client, voice_server_url: &str, req: VoiceSpeakRequest) {
    if req.text.trim().is_empty() {
        return;
    }
    let base = voice_server_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return;
    }
    let url = format!("{base}/speak");
    post_speak_request(client, &url, req);
}

fn post_speak_request(client: &reqwest::Client, url: &str, req: VoiceSpeakRequest) {
    let client = client.clone();
    let url = url.to_string();
    tokio::spawn(async move {
        match client.post(&url).json(&req).send().await {
            Ok(resp) => {
                let status = resp.status();
                // Drain body so the connection can return to the pool.
                let _ = resp.bytes().await;
                if !status.is_success() {
                    tracing::warn!(%status, %url, "voice server non-success");
                }
            }
            Err(err) => tracing::warn!(error = %err, %url, "voice server POST failed"),
        }
    });
}

