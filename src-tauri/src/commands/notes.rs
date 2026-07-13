// Note operations: list, read, write, create, delete, rename, file tree
// Block extraction, link extraction, card extraction.

use crate::error::{VaultError, VaultResult};
use crate::state::AppState;
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ---------- DTOs ----------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteMeta {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub modified: i64,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct NoteContent {
    pub path: String,
    pub raw: String,
    pub body: String,
    pub frontmatter: serde_json::Value,
    pub links: Vec<String>,
    pub tags: Vec<String>,
    pub title: String,
    pub modified: i64,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteLite {
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RawLink {
    pub from_block_id: Option<String>,
    pub to_alias: String,
    pub link_type: String,
    pub context: String,
    pub to_block_id: Option<String>,
}

// ---------- Block model ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub id: String,
    pub note_id: String,
    pub r#type: String,
    pub level: Option<u8>,
    pub content: String,
    pub content_hash: String,
    pub order_index: i64,
}

// ---------- Card model ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Card {
    pub id: String,
    pub note_path: String,
    pub block_id: Option<String>,
    pub card_type: String,
    pub question: String,
    pub answer: String,
    pub cloze_text: Option<String>,
    pub line_index: i64,
    pub difficulty: f64,
    pub stability: f64,
    pub interval_days: f64,
    pub reps: i64,
    pub lapses: i64,
    pub last_review_at: Option<i64>,
    pub next_review_at: i64,
    pub state: String,
    pub created_at: i64,
}

// ---------- Frontmatter helpers ----------

pub fn parse_frontmatter(raw: &str) -> (serde_json::Value, String) {
    if let Some(stripped) = raw.strip_prefix("---\n").or_else(|| raw.strip_prefix("---\r\n")) {
        if let Some(end) = find_frontmatter_end(stripped) {
            let yaml = &stripped[..end];
            let body_start = end + 4;
            let body = if stripped[body_start..].starts_with('\n') {
                &stripped[body_start + 1..]
            } else if stripped[body_start..].starts_with("\r\n") {
                &stripped[body_start + 2..]
            } else {
                &stripped[body_start..]
            };
            let fm: serde_json::Value =
                serde_yaml::from_str(yaml).unwrap_or(serde_json::Value::Null);
            return (fm, body.to_string());
        }
    }
    (serde_json::Value::Null, raw.to_string())
}

fn find_frontmatter_end(s: &str) -> Option<usize> {
    let mut i = 0;
    for line in s.split_inclusive('\n') {
        if line.trim() == "---" || line.trim() == "---\r" {
            return Some(i);
        }
        i += line.len();
        if i > 4096 {
            return None;
        }
    }
    None
}

fn extract_wikilinks(body: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\[\]\|]+?)(?:\|[^\]]+?)?\]\]").unwrap();
    re.captures_iter(body)
        .map(|c| c.get(1).unwrap().as_str().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn extract_inline_tags(body: &str) -> Vec<String> {
    let re = Regex::new(r"(?:^|\s)#([A-Za-z0-9_\-/\u{4E00}-\u{9FFF}]+)").unwrap();
    re.captures_iter(body)
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .collect()
}

pub fn derive_title(path: &Path, frontmatter: &serde_json::Value, body: &str) -> String {
    if let Some(t) = frontmatter.get("title").and_then(|v| v.as_str()) {
        if !t.is_empty() { return t.to_string(); }
    }
    for line in body.lines() {
        let trimmed = line.trim_start_matches('#').trim();
        if !trimmed.is_empty() { return trimmed.to_string(); }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string())
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

fn tags_from_frontmatter(fm: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = fm.get("tags").and_then(|v| v.as_array()) {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else if let Some(s) = fm.get("tags").and_then(|v| v.as_str()) {
        s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        Vec::new()
    }
}

pub fn collect_inline_tags_union(body: &str, fm: &serde_json::Value) -> Vec<String> {
    let mut all: Vec<String> = tags_from_frontmatter(fm);
    for t in extract_inline_tags(body) {
        if !all.contains(&t) { all.push(t); }
    }
    all.sort();
    all.dedup();
    all
}

// ---------- Block extraction ----------

pub fn extract_blocks(body: &str, note_id: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut order: i64 = 0;
    let lines: Vec<&str> = body.split('\n').collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() { i += 1; continue; }

        if line.trim_start().starts_with("```") {
            let mut content = String::new();
            content.push_str(line); content.push('\n'); i += 1;
            while i < lines.len() {
                content.push_str(lines[i]); content.push('\n');
                if lines[i].trim_start().starts_with("```") { i += 1; break; }
                i += 1;
            }
            push_block(&mut blocks, note_id, "code", None, &content, order);
            order += 1;
            continue;
        }

        if let Some(level) = heading_level(line) {
            let snippet = line.trim_start_matches('#').trim().to_string();
            push_block(&mut blocks, note_id, "heading", Some(level), &snippet, order);
            order += 1; i += 1; continue;
        }

        if line.trim_start().starts_with(">") {
            let mut content = String::new();
            while i < lines.len() && (lines[i].trim_start().starts_with(">") || lines[i].trim().is_empty()) {
                content.push_str(lines[i]); content.push('\n');
                if lines[i].trim().is_empty() { i += 1; break; }
                i += 1;
            }
            push_block(&mut blocks, note_id, "blockquote", None, &content, order);
            order += 1; continue;
        }

        let starts_list = line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ")
            || line.trim_start().starts_with("+ ")
            || (line.trim_start().len() > 0 && line.trim_start().chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                && line.trim_start().contains(". "));
        if starts_list {
            let mut content = String::new();
            while i < lines.len() && (
                lines[i].trim_start().starts_with("- ") || lines[i].trim_start().starts_with("* ")
                || lines[i].trim_start().starts_with("+ ") || lines[i].trim_start().starts_with("  - ")
                || (lines[i].trim_start().chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                    && lines[i].trim_start().contains(". "))
                || lines[i].trim().is_empty()
            ) {
                content.push_str(lines[i]); content.push('\n');
                if lines[i].trim().is_empty() { i += 1; break; }
                i += 1;
            }
            push_block(&mut blocks, note_id, "list", None, &content, order);
            order += 1; continue;
        }

        let mut content = String::new();
        while i < lines.len() && !lines[i].trim().is_empty()
            && !lines[i].trim_start().starts_with("```")
            && heading_level(lines[i]).is_none()
            && !lines[i].trim_start().starts_with(">")
            && !lines[i].trim_start().starts_with("- ")
            && !lines[i].trim_start().starts_with("* ")
            && !lines[i].trim_start().starts_with("+ ")
        {
            content.push_str(lines[i]); content.push('\n'); i += 1;
        }
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            push_block(&mut blocks, note_id, "paragraph", None, trimmed, order);
            order += 1;
        }
    }
    blocks
}

fn push_block(blocks: &mut Vec<Block>, note_id: &str, block_type: &str, level: Option<u8>, content: &str, order: i64) {
    let id = compute_block_id(block_type, content, order);
    let hash = short_hash(content);
    let snippet = content.chars().take(200).collect::<String>().trim().to_string();
    blocks.push(Block {
        id,
        note_id: note_id.to_string(),
        r#type: block_type.to_string(),
        level,
        content: snippet,
        content_hash: hash,
        order_index: order,
    });
}

fn heading_level(line: &str) -> Option<u8> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level >= 1 && level <= 6 {
        let after = &trimmed[level..];
        if after.is_empty() || after.starts_with(' ') { return Some(level as u8); }
    }
    None
}

fn compute_block_id(block_type: &str, content: &str, order: i64) -> String {
    let input = format!("{}|{}|{}", block_type, order, content);
    let h = short_hash(&input);
    format!("blk_{}", h)
}

pub fn short_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().iter().take(4).map(|b| format!("{:02x}", b)).collect()
}

// ---------- Link extraction ----------

pub fn extract_links(body: &str, blocks: &[Block]) -> Vec<RawLink> {
    let mut block_at: Vec<Option<String>> = vec![None; body.len()];
    let mut byte_pos = 0usize;
    let mut block_idx = 0usize;
    for line in body.split_inclusive('\n') {
        let line_end = byte_pos + line.len();
        let trimmed = line.trim();
        let mut current_id: Option<String> = None;
        if !trimmed.is_empty() && block_idx < blocks.len() {
            current_id = Some(blocks[block_idx].id.clone());
        }
        for p in byte_pos..line_end.min(body.len()) {
            block_at[p] = current_id.clone();
        }
        byte_pos = line_end;
        if trimmed.is_empty() {
            if block_idx + 1 < blocks.len() { block_idx += 1; }
        }
    }

    let wikilink_re = Regex::new(r"\[\[([^\[\]\|]+?)(?:\|([^\]]+?))?\]\]").unwrap();
    let block_ref_re = Regex::new(r"\(\(([A-Za-z0-9_\-]+)\)").unwrap();
    let trans_re = Regex::new(r"!\[\[([^\[\]\|]+?)(?:\|([^\]]+?))?\]\]").unwrap();

    let mut links = Vec::new();
    let mut seen_in_block: std::collections::HashSet<(Option<String>, String, String)> = std::collections::HashSet::new();

    for cap in trans_re.captures_iter(body) {
        let m = cap.get(0).unwrap();
        let target = cap.get(1).unwrap().as_str().trim().to_string();
        let ctx = context_around(body, m.start(), m.end(), 40);
        let bid = block_at.get(m.start()).cloned().flatten();
        let key = (bid.clone(), target.clone(), "transclusion".into());
        if seen_in_block.insert(key) {
            links.push(RawLink { from_block_id: bid, to_alias: target, link_type: "transclusion".into(), context: ctx, to_block_id: None });
        }
    }

    for cap in wikilink_re.captures_iter(body) {
        if body.as_bytes().get(cap.get(0).unwrap().start().saturating_sub(1)) == Some(&b'!') { continue; }
        let m = cap.get(0).unwrap();
        let raw_target = cap.get(1).unwrap().as_str().trim();
        let ctx = context_around(body, m.start(), m.end(), 40);
        let bid = block_at.get(m.start()).cloned().flatten();
        let (note_part, section_part, block_part) = parse_wikilink_target(raw_target);
        let key = (bid.clone(), raw_target.to_string(), "wiki".into());
        if seen_in_block.insert(key) {
            links.push(RawLink { from_block_id: bid, to_alias: note_part, link_type: "wiki".into(), context: ctx, to_block_id: block_part.or(section_part) });
        }
    }

    for cap in block_ref_re.captures_iter(body) {
        let m = cap.get(0).unwrap();
        let target = cap.get(1).unwrap().as_str().trim().to_string();
        let ctx = context_around(body, m.start(), m.end(), 40);
        let bid = block_at.get(m.start()).cloned().flatten();
        let key = (bid.clone(), target.clone(), "block_ref".into());
        if seen_in_block.insert(key) {
            links.push(RawLink { from_block_id: bid, to_alias: target.clone(), link_type: "block_ref".into(), context: ctx, to_block_id: Some(target) });
        }
    }
    links
}

fn parse_wikilink_target(target: &str) -> (String, Option<String>, Option<String>) {
    if let Some(hash_pos) = target.find('#') {
        let note = target[..hash_pos].trim().to_string();
        let rest = &target[hash_pos + 1..];
        if let Some(caret_pos) = rest.find('^') {
            (note, None, Some(rest[caret_pos + 1..].trim().to_string()))
        } else {
            (note, Some(rest.trim().to_string()), None)
        }
    } else {
        (target.to_string(), None, None)
    }
}

fn context_around(body: &str, start: usize, end: usize, padding: usize) -> String {
    let s = start.saturating_sub(padding);
    let e = (end + padding).min(body.len());
    let mut s = s; while s > 0 && !body.is_char_boundary(s) { s -= 1; }
    let mut e = e; while e < body.len() && !body.is_char_boundary(e) { e += 1; }
    body[s..e].replace('\n', " ").trim().to_string()
}

// ---------- Card extraction ----------

pub fn extract_cards(body: &str, note_path: &str) -> Vec<Card> {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut cards = Vec::new();
    let now = Utc::now().timestamp();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !is_list_item(trimmed) { continue; }
        let (card_type, content_part) = if let Some(rest) = strip_card_marker(trimmed, "#card-reverse") {
            ("reverse", rest)
        } else if let Some(rest) = strip_card_marker(trimmed, "#cloze") {
            ("cloze", rest)
        } else if let Some(rest) = strip_card_marker(trimmed, "#card") {
            ("basic", rest)
        } else { continue; };
        if content_part.trim().is_empty() { continue; }
        let base_indent = line.len() - trimmed.len();
        let mut answer_lines = Vec::new();
        let mut j = i + 1;
        while j < lines.len() {
            let next = lines[j];
            if next.trim().is_empty() { j += 1; continue; }
            let next_indent = next.len() - next.trim_start().len();
            if next_indent <= base_indent { break; }
            answer_lines.push(next.trim());
            j += 1;
        }
        let answer = answer_lines.join(" ").trim().to_string();
        let question = strip_list_prefix(content_part).trim().to_string();
        if question.is_empty() { continue; }
        let mut h = Sha256::new();
        h.update(format!("{}|{}|{}|{}", card_type, note_path, i, question).as_bytes());
        let id = format!("card_{}", h.finalize().iter().take(6).map(|b| format!("{:02x}", b)).collect::<String>());
        let (q, a, cloze) = if card_type == "cloze" {
            let stripped = strip_cloze_markers(&question);
            (stripped, String::new(), Some(question))
        } else {
            (question.clone(), answer, None)
        };
        let block_id = find_block_for_line(&lines, i);
        cards.push(Card {
            id, note_path: note_path.to_string(), block_id,
            card_type: card_type.to_string(), question: q, answer: a, cloze_text: cloze,
            line_index: i as i64, difficulty: 5.0, stability: 2.0, interval_days: 0.0,
            reps: 0, lapses: 0, last_review_at: None, next_review_at: now,
            state: "new".to_string(), created_at: now,
        });
    }
    cards
}

fn is_list_item(trimmed: &str) -> bool {
    if trimmed.is_empty() { return false; }
    let bytes = trimmed.as_bytes();
    if bytes[0] == b'-' || bytes[0] == b'*' || bytes[0] == b'+' {
        return trimmed.len() == 1 || bytes[1] == b' ';
    }
    if bytes[0].is_ascii_digit() {
        if let Some(dot_pos) = trimmed.find('.') { return dot_pos <= 3; }
    }
    false
}

fn strip_list_prefix(s: &str) -> &str {
    let trimmed = s.trim_start();
    if trimmed.is_empty() { return s; }
    let bytes = trimmed.as_bytes();
    if bytes[0] == b'-' || bytes[0] == b'*' || bytes[0] == b'+' {
        if trimmed.len() == 1 { return ""; }
        return &trimmed[2..];
    }
    if let Some(dot_pos) = trimmed.find('.') {
        if dot_pos <= 3 && trimmed.as_bytes().get(dot_pos + 1) == Some(&b' ') {
            return &trimmed[dot_pos + 2..];
        }
    }
    s
}

fn strip_card_marker<'a>(s: &'a str, marker: &str) -> Option<&'a str> {
    let s = s.trim_end();
    if let Some(idx) = s.rfind(marker) {
        if idx == 0 || s.as_bytes()[idx - 1] == b' ' {
            if idx + marker.len() == s.len() { return Some(&s[..idx]); }
        }
    }
    None
}

fn strip_cloze_markers(s: &str) -> String {
    let re = Regex::new(r"\{\{c\d+::([^}]*?)(?:::([^}]*?))?\}\}").unwrap();
    re.replace_all(s, "[$1]").to_string()
}

fn find_block_for_line(lines: &[&str], target_line: usize) -> Option<String> {
    let mut order: i64 = 0;
    let mut in_block = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() { in_block = false; continue; }
        if !in_block { in_block = true; }
        if i == target_line {
            let snippet = line.trim().chars().take(80).collect::<String>();
            let mut h = Sha256::new();
            h.update(format!("paragraph|{}|{}", order, snippet).as_bytes());
            let hash: String = h.finalize().iter().take(4).map(|b| format!("{:02x}", b)).collect();
            return Some(format!("blk_{}", hash));
        }
        order += 1;
    }
    None
}

// Helper used by write_note's card merge
pub fn db_card_from_row(r: &rusqlite::Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get(0)?, note_path: r.get(1)?, block_id: r.get(2)?, card_type: r.get(3)?,
        question: r.get(4)?, answer: r.get(5)?, cloze_text: r.get(6)?, line_index: r.get(7)?,
        difficulty: r.get(8)?, stability: r.get(9)?, interval_days: r.get(10)?,
        reps: r.get(11)?, lapses: r.get(12)?, last_review_at: r.get(13)?,
        next_review_at: r.get(14)?, state: r.get(15)?, created_at: r.get(16)?,
    })
}

// ---------- Commands ----------

#[tauri::command]
pub fn list_notes(state: tauri::State<AppState>, dir: Option<String>) -> VaultResult<Vec<NoteMeta>> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let results = list_notes_inner(&base, dir.as_deref())?;
    if let Err(e) = state.with_db(|db| {
        let sync_data = results.iter().map(|m| {
            let full = base.join(&m.path);
            let raw = std::fs::read_to_string(&full).unwrap_or_default();
            let (_fm, body) = parse_frontmatter(&raw);
            let tags_joined = m.tags.join(" ");
            let mut h = Sha256::new();
            Digest::update(&mut h, raw.as_bytes());
            let hash = h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
            (m.path.clone(), m.title.clone(), body, tags_joined, m.modified, m.size, hash)
        }).collect::<Vec<_>>();
        db.sync_with_files(&sync_data)
    }) {
        log::warn!("index sync failed: {}", e);
    }
    Ok(results)
}

pub fn list_notes_inner(base: &Path, dir: Option<&str>) -> VaultResult<Vec<NoteMeta>> {
    let scan_root = match dir {
        Some(d) if !d.is_empty() => base.join(d),
        _ => base.to_path_buf(),
    };
    let mut results = Vec::new();
    for entry in WalkDir::new(&scan_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') && e.depth() == 0)
                && !(e.depth() > 0 && name.starts_with('.'))
                && name != "node_modules"
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() { continue; }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("md") { continue; }
        let rel = entry.path().strip_prefix(base).map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());
        let raw = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let (fm, body) = parse_frontmatter(&raw);
        let title = derive_title(entry.path(), &fm, &body);
        let tags = collect_inline_tags_union(&body, &fm);
        let meta = entry.metadata().map_err(|e| VaultError::Other(format!("metadata error: {}", e)))?;
        let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64).unwrap_or(0);
        results.push(NoteMeta { path: rel, title, tags, modified, size: meta.len() });
    }
    results.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(results)
}

#[tauri::command]
pub fn read_note(state: tauri::State<AppState>, path: String) -> VaultResult<NoteContent> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let full_path = PathBuf::from(&info.path).join(&path);
    if !full_path.exists() { return Err(VaultError::NoteNotFound(path)); }
    let raw = std::fs::read_to_string(&full_path)?;
    let (fm, body) = parse_frontmatter(&raw);
    let title = derive_title(&full_path, &fm, &body);
    let links = extract_wikilinks(&body);
    let tags = collect_inline_tags_union(&body, &fm);
    let meta = std::fs::metadata(&full_path)?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64).unwrap_or(0);
    Ok(NoteContent { path, raw, body, frontmatter: fm, links, tags, title, modified, size: meta.len() })
}

#[tauri::command]
pub fn write_note(state: tauri::State<AppState>, path: String, content: String) -> VaultResult<NoteMeta> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let full_path = PathBuf::from(&info.path).join(&path);
    if let Some(parent) = full_path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&full_path, &content)?;

    let (fm, body) = parse_frontmatter(&content);
    let title = derive_title(&full_path, &fm, &body);
    let tags = collect_inline_tags_union(&body, &fm);
    let hash = sha256_hex(&content);
    let modified = Utc::now().timestamp();
    let size = content.len() as u64;

    state.with_db(|db| {
        db.upsert_note(&path, &title, &body, &tags.join(" "), modified, size, &hash)?;
        let blocks = extract_blocks(&body, &path);
        let raw_links = extract_links(&body, &blocks);
        db.replace_blocks(&blocks)?;
        db.insert_links(&path, &raw_links)?;
        let new_cards = extract_cards(&body, &path);
        let mut existing: std::collections::HashMap<String, Card> = std::collections::HashMap::new();
        {
            let mut stmt = db.conn.prepare(
                "SELECT id, note_path, block_id, card_type, question, answer, cloze_text, line_index,
                        difficulty, stability, interval_days, reps, lapses, last_review_at,
                        next_review_at, state, created_at
                 FROM cards WHERE note_path = ?1"
            )?;
            let rows = stmt.query_map(rusqlite::params![&path], db_card_from_row)?;
            for r in rows { if let Ok(c) = r { existing.insert(c.id.clone(), c); } }
        }
        let now = Utc::now().timestamp();
        let merged: Vec<Card> = new_cards.into_iter().map(|mut c| {
            if let Some(prev) = existing.get(&c.id) {
                c.difficulty = prev.difficulty;
                c.stability = prev.stability;
                c.interval_days = prev.interval_days;
                c.reps = prev.reps;
                c.lapses = prev.lapses;
                c.last_review_at = prev.last_review_at;
                c.next_review_at = prev.next_review_at;
                c.state = prev.state.clone();
                c.created_at = prev.created_at;
            } else {
                c.created_at = now;
            }
            c
        }).collect();
        db.replace_cards(&merged)?;
        Ok::<(), VaultError>(())
    })?;

    // Auto-commit if this is a git repo
    let _ = crate::git::auto_commit_note(&PathBuf::from(&info.path), &path);

    Ok(NoteMeta { path, title, tags, modified, size })
}

#[tauri::command]
pub fn create_note(state: tauri::State<AppState>, path: String, template: Option<String>) -> VaultResult<NoteMeta> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let full_path = PathBuf::from(&info.path).join(&path);
    if full_path.exists() { return Err(VaultError::Other(format!("Note already exists: {}", path))); }
    if let Some(parent) = full_path.parent() { std::fs::create_dir_all(parent)?; }
    let content = template.unwrap_or_else(|| {
        let stem = full_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "untitled".to_string());
        format!("# {}\n\n", stem)
    });
    std::fs::write(&full_path, &content)?;
    let (fm, body) = parse_frontmatter(&content);
    let title = derive_title(&full_path, &fm, &body);
    let tags = collect_inline_tags_union(&body, &fm);
    let hash = sha256_hex(&content);
    let modified = Utc::now().timestamp();
    let size = content.len() as u64;
    state.with_db(|db| {
        db.upsert_note(&path, &title, &body, &tags.join(" "), modified, size, &hash)?;
        let blocks = extract_blocks(&body, &path);
        let links = extract_links(&body, &blocks);
        db.replace_blocks(&blocks)?;
        db.insert_links(&path, &links)?;
        let cards = extract_cards(&body, &path);
        db.replace_cards(&cards)?;
        Ok::<(), VaultError>(())
    })?;
    Ok(NoteMeta { path, title, tags, modified, size })
}

#[tauri::command]
pub fn create_note_simple(state: tauri::State<AppState>, name: String) -> VaultResult<NoteMeta> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let clean = name.trim().trim_end_matches('/').trim_end_matches('\\');
    if clean.is_empty() { return Err(VaultError::Other("笔记名不能为空".into())); }
    let stem = clean.trim_end_matches(".md");
    let filename = format!("{}.md", stem);
    let target_dir = pick_inbox_dir(&base).unwrap_or_else(|| base.clone());
    let full_path = target_dir.join(&filename);
    let rel_path = full_path.strip_prefix(&base).map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| filename.clone());
    if full_path.exists() { return Err(VaultError::Other(format!("笔记已存在: {}", rel_path))); }
    if let Some(parent) = full_path.parent() { std::fs::create_dir_all(parent)?; }
    let content = format!("# {}\n\n", stem);
    std::fs::write(&full_path, &content)?;
    let (fm, body) = parse_frontmatter(&content);
    let title = derive_title(&full_path, &fm, &body);
    let tags = collect_inline_tags_union(&body, &fm);
    let hash = sha256_hex(&content);
    let modified = Utc::now().timestamp();
    let size = content.len() as u64;
    state.with_db(|db| {
        db.upsert_note(&rel_path, &title, &body, &tags.join(" "), modified, size, &hash)?;
        let blocks = extract_blocks(&body, &rel_path);
        let links = extract_links(&body, &blocks);
        db.replace_blocks(&blocks)?;
        db.insert_links(&rel_path, &links)?;
        let cards = extract_cards(&body, &rel_path);
        db.replace_cards(&cards)?;
        Ok::<(), VaultError>(())
    })?;
    Ok(NoteMeta { path: rel_path, title, tags, modified, size })
}

fn pick_inbox_dir(base: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(base).ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("0-") && e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            return Some(e.path());
        }
    }
    None
}

#[tauri::command]
pub fn delete_note(state: tauri::State<AppState>, path: String) -> VaultResult<()> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let full_path = PathBuf::from(&info.path).join(&path);
    if !full_path.exists() { return Err(VaultError::NoteNotFound(path)); }
    std::fs::remove_file(&full_path)?;
    state.with_db(|db| db.remove_note(&path))?;
    Ok(())
}

#[tauri::command]
pub fn rename_note(state: tauri::State<AppState>, old_path: String, new_path: String) -> VaultResult<()> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let old_full = base.join(&old_path);
    let new_full = base.join(&new_path);
    if !old_full.exists() { return Err(VaultError::NoteNotFound(old_path)); }
    if new_full.exists() { return Err(VaultError::Other(format!("Target exists: {}", new_path))); }
    if let Some(parent) = new_full.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::rename(&old_full, &new_full)?;
    state.with_db(|db| db.remove_note(&old_path))?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct ReindexResult {
    pub added: usize,
    pub removed: usize,
    pub total: usize,
}

#[tauri::command]
pub fn reindex_vault(state: tauri::State<AppState>) -> VaultResult<ReindexResult> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let mut sync_data: Vec<(String, String, String, String, i64, u64, String)> = Vec::new();
    for entry in WalkDir::new(&base).follow_links(false).into_iter()
        .filter_entry(|e| { let n = e.file_name().to_string_lossy(); !(e.depth() > 0 && n.starts_with('.')) && n != "node_modules" })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() { continue; }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("md") { continue; }
        let rel = entry.path().strip_prefix(&base).map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());
        let raw = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let (fm, body) = parse_frontmatter(&raw);
        let title = derive_title(entry.path(), &fm, &body);
        let tags = collect_inline_tags_union(&body, &fm);
        let meta = entry.metadata().ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let hash = sha256_hex(&raw);
        sync_data.push((rel, title, body.clone(), tags.join(" "), modified, size, hash));
    }
    let (added, removed) = state.with_db(|db| db.sync_with_files(&sync_data))?;
    for (path, _title, body, _tags, _m, _s, _h) in &sync_data {
        let blocks = extract_blocks(body, path);
        let raw_links = extract_links(body, &blocks);
        let cards = extract_cards(body, path);
        state.with_db(|db| {
            db.replace_blocks(&blocks)?;
            db.insert_links(path, &raw_links)?;
            db.replace_cards(&cards)?;
            Ok::<(), VaultError>(())
        })?;
    }
    Ok(ReindexResult { added, removed, total: sync_data.len() })
}

#[tauri::command]
pub fn get_file_tree(state: tauri::State<AppState>, dir: Option<String>) -> VaultResult<Vec<TreeNode>> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let scan_root = match &dir {
        Some(d) if !d.is_empty() => base.join(d),
        _ => base.clone(),
    };
    fn build(p: &Path, base: &Path) -> Option<TreeNode> {
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if p != base && name.starts_with('.') { return None; }
        if name == "node_modules" { return None; }
        let rel = p.strip_prefix(base).map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| p.to_string_lossy().to_string());
        let is_dir = p.is_dir();
        let mut children = Vec::new();
        if is_dir {
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
                entries.sort_by_key(|e| (!e.file_type().map(|t| t.is_dir()).unwrap_or(false), e.file_name().to_string_lossy().to_lowercase()));
                for entry in entries { if let Some(child) = build(&entry.path(), base) { children.push(child); } }
            }
        }
        Some(TreeNode { name: if name.is_empty() { p.to_string_lossy().to_string() } else { name }, path: rel, is_dir, children })
    }
    Ok(vec![build(&scan_root, &base).unwrap_or(TreeNode { name: scan_root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), path: "".to_string(), is_dir: true, children: vec![] })])
}
