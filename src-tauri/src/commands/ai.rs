// AI integration: OpenAI-compatible chat completions via std::process::Command (curl).
// Two providers supported:
//   - "ollama"   -> POST {base}/api/chat                (no api key required)
//   - "openai"   -> POST {base}/chat/completions         (Bearer auth)
// Streams tokens by polling NDJSON / chunked responses? — for MVP, we just buffer the
// full response and emit it in one go via the `ai-done` event. The frontend treats
// that as a single delta, which keeps the implementation simple while still feeling
// responsive (the LLM call is the slow part either way).

use crate::db::SearchHit;
use crate::error::{VaultError, VaultResult};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

// ---------------------------------------------------------------------------
// Request / response types (camelCase to match TS contract)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub provider: String,        // "ollama" | "openai"
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub system_prompt: String,
    pub question: String,
    pub top_k: usize,
    pub temperature: f32,
    pub max_tokens: u32,
    pub scope_paths: Vec<String>,
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsRequest {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiSource {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub block_id: Option<String>,
    pub score: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatResult {
    pub content: String,
    pub sources: Vec<AiSource>,
}

#[derive(Debug, Serialize, Clone)]
struct StreamEventSources {
    request_id: String,
    sources: Vec<AiSource>,
}

#[derive(Debug, Serialize, Clone)]
struct StreamEventChunk {
    request_id: String,
    delta: String,
}

#[derive(Debug, Serialize, Clone)]
struct StreamEventDone {
    request_id: String,
    content: String,
}

#[derive(Debug, Serialize, Clone)]
struct StreamEventError {
    request_id: String,
    error: String,
}

// ---------------------------------------------------------------------------
// Cancellation registry (kept in Tauri state extension)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct AiCancelRegistry {
    inner: Mutex<std::collections::HashMap<String, bool>>,
}

impl AiCancelRegistry {
    fn cancel(&self, id: &str) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(id.to_string(), true);
        }
    }
    fn is_cancelled(&self, id: &str) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.get(id).copied())
            .unwrap_or(false)
    }
    fn clear(&self, id: &str) {
        if let Ok(mut g) = self.inner.lock() {
            g.remove(id);
        }
    }
}

pub fn build_ai_state() -> AiCancelRegistry {
    AiCancelRegistry::default()
}

// ---------------------------------------------------------------------------
// Retrieval (FTS top-k → context block)
// ---------------------------------------------------------------------------

fn retrieve_context(
    state: &AppState,
    base: &Path,
    question: &str,
    top_k: usize,
    scope_paths: &[String],
) -> VaultResult<Vec<AiSource>> {
    let hits: Vec<SearchHit> = state.with_db(|db| db.search(question, top_k as u32))?;
    let mut sources = Vec::new();
    for h in hits {
        if !scope_paths.is_empty() && !scope_paths.iter().any(|p| h.path.starts_with(p)) {
            continue;
        }
        let snippet = h.snippet.clone();
        sources.push(AiSource {
            path: h.path,
            title: h.title,
            snippet,
            block_id: None,
            score: h.rank,
        });
    }
    Ok(sources)
}

fn build_messages(req: &ChatRequest, sources: &[AiSource], base: &Path) -> Vec<serde_json::Value> {
    // Build the context block from sources (full body, truncated to keep prompt sane).
    let mut context_text = String::new();
    for s in sources {
        let body = std::fs::read_to_string(base.join(&s.path))
            .unwrap_or_default();
        // Strip frontmatter to save tokens.
        let body = if let Some(idx) = body.find("---") {
            if body[idx..].starts_with("---") {
                if let Some(end) = body[idx + 3..].find("\n---") {
                    body[idx + 3 + end + 4..].trim_start_matches('\n').to_string()
                } else {
                    body.clone()
                }
            } else {
                body.clone()
            }
        } else {
            body.clone()
        };
        let body: String = body.chars().take(1500).collect();
        context_text.push_str(&format!(
            "### {} [{}]\n{}\n\n",
            if s.title.is_empty() { &s.path } else { &s.title },
            s.path,
            body,
        ));
    }
    if context_text.is_empty() {
        context_text = "(本库中没检索到相关内容)".into();
    }

    let user_prompt = format!(
        "# 笔记上下文（来自本地检索）\n\n{}\n\n# 用户问题\n\n{}",
        context_text, req.question
    );

    let mut msgs: Vec<serde_json::Value> = Vec::new();
    msgs.push(serde_json::json!({
        "role": "system",
        "content": req.system_prompt,
    }));
    for h in &req.history {
        msgs.push(serde_json::json!({
            "role": h.role,
            "content": h.content,
        }));
    }
    msgs.push(serde_json::json!({
        "role": "user",
        "content": user_prompt,
    }));
    msgs
}

// ---------------------------------------------------------------------------
// Provider adapters — return either (url, extra_headers, body_json)
// ---------------------------------------------------------------------------

fn build_call(
    req: &ChatRequest,
    messages: Vec<serde_json::Value>,
) -> VaultResult<(String, Vec<String>, serde_json::Value)> {
    let base = req.base_url.trim_end_matches('/').to_string();
    match req.provider.as_str() {
        "ollama" => {
            let url = format!("{}/api/chat", base);
            let body = serde_json::json!({
                "model": req.model,
                "stream": false,
                "options": {
                    "temperature": req.temperature,
                    "num_predict": req.max_tokens,
                },
                "messages": messages,
            });
            Ok((url, vec![], body))
        }
        "openai" | _ => {
            let url = if base.ends_with("/chat/completions") {
                base
            } else {
                format!("{}/chat/completions", base)
            };
            let body = serde_json::json!({
                "model": req.model,
                "temperature": req.temperature,
                "max_tokens": req.max_tokens,
                "messages": messages,
            });
            let mut headers = vec!["Content-Type: application/json".to_string()];
            if !req.api_key.is_empty() {
                headers.push(format!("Authorization: Bearer {}", req.api_key));
            }
            Ok((url, headers, body))
        }
    }
}

fn extract_content(provider: &str, raw: &str) -> String {
    let v: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return raw.to_string(),
    };
    match provider {
        "ollama" => v
            .pointer("/message/content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        _ => v
            .pointer("/choices/0/message/content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn ai_list_models(req: ListModelsRequest) -> VaultResult<Vec<String>> {
    let base = req.base_url.trim_end_matches('/').to_string();
    let auth_header = if req.provider != "ollama" && !req.api_key.is_empty() {
        format!("Authorization: Bearer {}", req.api_key)
    } else {
        String::new()
    };
    let url = match req.provider.as_str() {
        "ollama" => format!("{}/api/tags", base),
        _ => format!("{}/models", base),
    };
    let mut args: Vec<String> = vec!["-sS".into(), "-X".into(), "GET".into()];
    if !auth_header.is_empty() {
        args.push("-H".into());
        args.push(auth_header);
    }

    let output = Command::new("curl")
        .args(&args)
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| VaultError::Other(format!("curl not found: {}", e)))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(VaultError::Other(format!("list models failed: {}", err)));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| VaultError::Other(format!("bad list response: {} - {}", e, &raw[..raw.len().min(200)])))?;
    let models: Vec<String> = match req.provider.as_str() {
        "ollama" => v
            .pointer("/models")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
            .unwrap_or_default(),
        _ => v
            .pointer("/data")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|m| m.get("id").and_then(|n| n.as_str()).map(String::from)).collect())
            .unwrap_or_default(),
    };
    Ok(models)
}

#[tauri::command]
pub fn ai_chat(
    state: tauri::State<AppState>,
    req: ChatRequest,
) -> VaultResult<ChatResult> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = std::path::PathBuf::from(&info.path);

    let sources = retrieve_context(&state, &base, &req.question, req.top_k, &req.scope_paths)?;
    let messages = build_messages(&req, &sources, &base);
    let (url, headers, body) = build_call(&req, messages)?;

    let body_str = body.to_string();
    let mut args: Vec<String> = vec![
        "-sS".into(),
        "-X".into(),
        "POST".into(),
        "--max-time".into(),
        "120".into(),
    ];
    for h in &headers {
        args.push("-H".into());
        args.push(h.clone());
    }
    args.push("-d".into());
    args.push(body_str);

    let output = Command::new("curl")
        .args(&args)
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| VaultError::Other(format!("curl not found: {}", e)))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        return Err(VaultError::Other(format!(
            "AI call failed: {} {}",
            err,
            &out[..out.len().min(400)]
        )));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let content = extract_content(&req.provider, &raw);
    Ok(ChatResult { content, sources })
}

#[tauri::command]
pub fn ai_chat_stream(
    app: AppHandle,
    state: tauri::State<AppState>,
    registry: State<'_, AiCancelRegistry>,
    req: ChatRequest,
    request_id: String,
) -> VaultResult<()> {
    registry.clear(&request_id);
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = std::path::PathBuf::from(&info.path);

    let sources = match retrieve_context(&state, &base, &req.question, req.top_k, &req.scope_paths) {
        Ok(s) => s,
        Err(e) => {
            let _ = app.emit(
                "ai-error",
                StreamEventError { request_id: request_id.clone(), error: e.to_string() },
            );
            return Err(e);
        }
    };
    let _ = app.emit(
        "ai-sources",
        StreamEventSources { request_id: request_id.clone(), sources: sources.clone() },
    );

    let messages = build_messages(&req, &sources, &base);
    let (url, headers, body) = build_call(&req, messages)?;
    let body_str = body.to_string();

    let mut args: Vec<String> = vec![
        "-sS".into(),
        "-X".into(),
        "POST".into(),
        "--max-time".into(),
        "180".into(),
    ];
    for h in &headers {
        args.push("-H".into());
        args.push(h.clone());
    }
    args.push("-d".into());
    args.push(body_str);

    let output = Command::new("curl")
        .args(&args)
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| VaultError::Other(format!("curl not found: {}", e)))?;

    if registry.is_cancelled(&request_id) {
        registry.clear(&request_id);
        return Ok(());
    }
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let msg = format!(
            "AI call failed: {} {}",
            err,
            &out[..out.len().min(400)]
        );
        let _ = app.emit(
            "ai-error",
            StreamEventError { request_id: request_id.clone(), error: msg.clone() },
        );
        return Err(VaultError::Other(msg));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let content = extract_content(&req.provider, &raw);

    // For MVP, emit as a single chunk then done.
    if !content.is_empty() {
        let _ = app.emit(
            "ai-chunk",
            StreamEventChunk { request_id: request_id.clone(), delta: content.clone() },
        );
    }
    let _ = app.emit(
        "ai-done",
        StreamEventDone { request_id, content },
    );
    Ok(())
}

#[tauri::command]
pub fn ai_cancel(
    registry: State<'_, AiCancelRegistry>,
    request_id: String,
) {
    registry.cancel(&request_id);
}

/// Helper used during command registration to set up the cancel registry on app state.
pub fn install_ai_state(app: &AppHandle) {
    app.manage(build_ai_state());
}

// ---------------------------------------------------------------------------
// Card generation: ask the LLM to produce Q&A pairs from a note's body.
// Returns a list of (question, answer) pairs. The caller decides which to keep
// and inserts them via the cards module (so FSRS state is preserved).
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenerateCardsRequest {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub note_path: String,
    pub count: u8,
    pub language: String,        // e.g. "中文" / "English"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCard {
    pub question: String,
    pub answer: String,
    pub card_type: String,       // "basic" | "reverse" | "cloze"
    pub source: String,          // which heading / line it came from
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenerateCardsResult {
    pub cards: Vec<GeneratedCard>,
    pub raw: String,             // raw LLM output (for debugging)
}

#[tauri::command]
pub fn ai_generate_cards(
    state: tauri::State<AppState>,
    req: GenerateCardsRequest,
) -> VaultResult<GenerateCardsResult> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = std::path::PathBuf::from(&info.path);
    let abs = base.join(&req.note_path);
    let raw = std::fs::read_to_string(&abs)
        .map_err(|e| VaultError::Other(format!("读笔记失败: {}", e)))?;
    let (_fm, body) = crate::commands::notes::parse_frontmatter(&raw);
    // Cap body length to keep prompt small.
    let body = body.chars().take(6000).collect::<String>();
    let count = req.count.clamp(1, 30);

    let system = format!(
        "你是一个为 {language} 学习者生成闪卡的助手。\n\
         阅读用户提供的笔记，提取 {count} 张高质量的 Q&A 闪卡，遵循：\n\
         - 答案应该**自包含**，脱离笔记也能理解（不要写\"见上文\"）\n\
         - 题目应**具体、可问**，避免太宽泛（如\"什么是 X\"）\n\
         - 优先选**核心概念、易错点、对比、例子**\n\
         - 答案简洁（一般 1-3 句），不复制整段\n\
         - 不要编造笔记里没有的信息\n\
         - 笔记里的 `#card` 标记已经存在，无需重复\n\
         - 使用 {language} 输出\n\
         \n\
         严格按以下 JSON 数组格式返回（不要 markdown 代码块、不要任何解释文字）：\n\
         [\n  \
           {{\"question\": \"...\", \"answer\": \"...\", \"cardType\": \"basic\"}},\n  \
           {{\"question\": \"...\", \"answer\": \"...\", \"cardType\": \"reverse\"}}\n\
         ]\n\
         cardType 可选: basic（默认）/ reverse（双向卡）/ cloze（填空，仅在确实适合时用）",
        count = count,
        language = req.language
    );
    let user_prompt = format!("# 笔记内容\n\n{}\n\n请生成 {} 张闪卡。", body, count);

    let chat_req = ChatRequest {
        provider: req.provider.clone(),
        model: req.model.clone(),
        base_url: req.base_url.clone(),
        api_key: req.api_key.clone(),
        system_prompt: system,
        question: user_prompt,
        top_k: 0,
        temperature: 0.4,
        max_tokens: 2048,
        scope_paths: vec![],
        history: vec![],
    };

    // Reuse the shared message builder (empty sources, just system + user)
    let messages = vec![
        serde_json::json!({"role": "system", "content": chat_req.system_prompt}),
        serde_json::json!({"role": "user", "content": chat_req.question}),
    ];
    let (url, headers, body) = build_call(&chat_req, messages)?;
    let body_str = body.to_string();
    let mut args: Vec<String> = vec![
        "-sS".into(), "-X".into(), "POST".into(),
        "--max-time".into(), "120".into(),
    ];
    for h in &headers {
        args.push("-H".into());
        args.push(h.clone());
    }
    args.push("-d".into());
    args.push(body_str);
    let output = Command::new("curl")
        .args(&args)
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| VaultError::Other(format!("无法启动 curl: {}", e)))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(VaultError::Other(format!("LLM 失败: {}", err)));
    }
    let raw_resp = String::from_utf8_lossy(&output.stdout);
    let content = extract_content(&req.provider, &raw_resp);

    // Try to parse the JSON array from the response. Strip ```json fences if any.
    let json_text = strip_code_fence(&content);
    let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&json_text);
    let mut cards: Vec<GeneratedCard> = vec![];
    match parsed {
        Ok(arr) => {
            for item in arr {
                let q = item.get("question").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let a = item.get("answer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let t = item.get("cardType")
                    .or_else(|| item.get("card_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("basic")
                    .to_string();
                if q.is_empty() || a.is_empty() { continue; }
                let allowed = matches!(t.as_str(), "basic" | "reverse" | "cloze");
                cards.push(GeneratedCard {
                    question: q,
                    answer: a,
                    card_type: if allowed { t } else { "basic".into() },
                    source: String::new(),
                });
            }
        }
        Err(e) => {
            return Err(VaultError::Other(format!(
                "LLM 返回的不是合法 JSON: {}\n---\n{}",
                e, &content[..content.len().min(800)]
            )));
        }
    }
    if cards.is_empty() {
        return Err(VaultError::Other("LLM 没返回任何卡片".into()));
    }
    Ok(GenerateCardsResult { cards, raw: content })
}

fn strip_code_fence(s: &str) -> String {
    let t = s.trim();
    // Remove leading ```json or ```
    let stripped = if t.starts_with("```") {
        let after = t.find('\n').map(|i| &t[i + 1..]).unwrap_or("");
        let trimmed = after.trim_end_matches("```").trim_end();
        trimmed.to_string()
    } else {
        t.to_string()
    };
    // If it doesn't start with `[` try to find the first `[` and last `]`.
    if let Some(start) = stripped.find('[') {
        if let Some(end) = stripped.rfind(']') {
            if end > start {
                return stripped[start..=end].to_string();
            }
        }
    }
    stripped
}
