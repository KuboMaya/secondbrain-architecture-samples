//! Portfolio sample — domains/ai (Gemini client + punctuation pseudo-stream for TTS.)
//! Source: src-tauri/src/domains/ai/client.rs

//! Gemini API client. Missing keys / `cfg(test)` never hit the network.

use serde_json::Value;

const GEMINI_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-flash-latest:generateContent";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeminiResult {
    pub text: String,
    pub token_usage: i64,
}

/// Resolve API key: explicit arg → `GEMINI_API_KEY` env (dotenv-loaded in main).
pub fn resolve_api_key(passed: Option<&str>) -> Option<String> {
    if let Some(key) = passed.map(str::trim).filter(|k| !k.is_empty()) {
        return Some(key.to_string());
    }
    std::env::var("GEMINI_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// Call Gemini (or return a deterministic stub).
pub async fn generate_content(api_key: Option<&str>, prompt: &str) -> GeminiResult {
    let resolved = resolve_api_key(api_key);
    if should_stub(resolved.as_deref()) {
        return stub_response(prompt);
    }

    let key = resolved.expect("checked by should_stub");
    let client = reqwest::Client::new();
    match client
        .post(GEMINI_URL)
        .query(&[("key", key.as_str())])
        .json(&serde_json::json!({
            "contents": [{ "parts": [{ "text": prompt }] }]
        }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_else(|_| "{}".into());
            let text = extract_gemini_text(&body).unwrap_or(body.clone());
            let token_usage = estimate_tokens(prompt, &text);
            GeminiResult { text, token_usage }
        }
        Ok(resp) => {
            tracing::warn!(status = %resp.status(), "gemini non-success; falling back to stub");
            stub_response(prompt)
        }
        Err(err) => {
            tracing::warn!(error = %err, "gemini request failed; falling back to stub");
            stub_response(prompt)
        }
    }
}

/// Generate then emit punctuation-sized pseudo-stream deltas (plan: chunk → speak).
/// `on_delta` is called for each speakable fragment in order.
pub async fn generate_content_stream<F>(
    api_key: Option<&str>,
    prompt: &str,
    mut on_delta: F,
) -> GeminiResult
where
    F: FnMut(&str),
{
    let result = generate_content(api_key, prompt).await;
    for chunk in pseudo_stream_chunks(&result.text) {
        on_delta(&chunk);
    }
    result
}

/// Split text into punctuation-terminated chunks; trailing remainder is last chunk.
fn pseudo_stream_chunks(text: &str) -> Vec<String> {
    const PUNCT: &[char] = &['。', '、', '！', '？', '!', '?', '．', '｡', '\n'];
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        buf.push(ch);
        if PUNCT.contains(&ch) {
            let piece = std::mem::take(&mut buf);
            if !piece.trim().is_empty() {
                out.push(piece);
            }
        }
    }
    if !buf.trim().is_empty() {
        out.push(buf);
    }
    if out.is_empty() && !text.is_empty() {
        out.push(text.to_string());
    }
    out
}

fn should_stub(api_key: Option<&str>) -> bool {
    cfg!(test) || api_key.map(str::trim).filter(|k| !k.is_empty()).is_none()
}

/// Stub used in tests / offline: keyword-extraction prompts get a short title.
fn stub_response(prompt: &str) -> GeminiResult {
    if prompt.contains("モーニングブリーフィング") || prompt.contains("朝の読み上げ") {
        return GeminiResult {
            text: "[joy:0.4]おはようございます。今日も穏やかな一日になりそうです。天気と予定を確認して、無理なく進みましょう。".into(),
            token_usage: 16,
        };
    }
    if prompt.contains("英語学習用") || prompt.contains("英語に翻訳") {
        return GeminiResult {
            text: "[EN stub] A natural English translation for language study.".into(),
            token_usage: 12,
        };
    }
    if prompt.contains("自動分類") || prompt.contains("create_new_index") {
        return GeminiResult {
            text: r#"{"tags":["自動分類","要確認"],"index_name":"一般","create_new_index":false,"rationale":"（stub）内容から一般フォルダと補完タグを提案"}"#.into(),
            token_usage: 10,
        };
    }
    if prompt.contains("辞書に登録") || prompt.contains("\"keyword\"") {
        return GeminiResult {
            text: r#"{"keyword":"AI題名抽出","tags":["AI","自動登録"]}"#.into(),
            token_usage: 8,
        };
    }
    if prompt.contains("題名") {
        return GeminiResult {
            text: "AI題名抽出".into(),
            token_usage: 4,
        };
    }
    GeminiResult {
        text: format!(
            "(stub) ご質問「{}」について、要点を整理すると次のとおりです。",
            prompt.chars().take(40).collect::<String>()
        ),
        token_usage: 0,
    }
}

fn extract_gemini_text(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.pointer("/candidates/0/content/parts/0/text")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

fn estimate_tokens(prompt: &str, response: &str) -> i64 {
    ((prompt.len() + response.len()) / 4) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_without_key() {
        let r = generate_content(None, "hello").await;
        assert!(r.text.contains("stub"));
    }

    #[tokio::test]
    async fn stub_keyword_prompt() {
        let r = generate_content(None, "題名(keyword)を10文字以内で抽出").await;
        assert!(r.text.contains("AI題名抽出"));
    }

    #[tokio::test]
    async fn stream_emits_punctuated_chunks() {
        let mut parts = Vec::new();
        let r = generate_content_stream(None, "モーニングブリーフィング 朝の読み上げ", |d| {
            parts.push(d.to_string());
        })
        .await;
        assert!(!parts.is_empty());
        assert_eq!(parts.concat(), r.text);
        assert!(r.text.contains("おはよう"));
    }

    #[test]
    fn resolve_reads_env_when_passed_none() {
        // SAFETY: test-only; isolated process for this unit test binary.
        std::env::remove_var("GEMINI_API_KEY");
        assert!(resolve_api_key(None).is_none());
        assert_eq!(
            resolve_api_key(Some(" from-arg ")).as_deref(),
            Some("from-arg")
        );
    }
}
