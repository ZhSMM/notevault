// Test helpers - exposed under `test_helpers` module for use in integration tests.
// Not part of the public API; just convenient functions for verifying the
// backend logic without going through Tauri's IPC.

use crate::commands::notes::{
    collect_inline_tags_union, derive_title, extract_blocks, extract_cards, extract_links,
    parse_frontmatter, Block, Card, RawLink,
};
use crate::db::Database;
use crate::error::VaultResult;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::Path;

pub use crate::commands::notes::NoteMeta;

pub fn parse_test_frontmatter_pub(s: &str) -> (serde_json::Value, String) {
    parse_frontmatter(s)
}
pub fn extract_blocks_pub(body: &str, note_id: &str) -> Vec<Block> {
    extract_blocks(body, note_id)
}
pub fn extract_links_pub(body: &str, blocks: &[Block]) -> Vec<RawLink> {
    extract_links(body, blocks)
}
pub fn extract_cards_pub(body: &str, note_path: &str) -> Vec<Card> {
    extract_cards(body, note_path)
}

pub fn open_db_for_test(vault: &Path) -> VaultResult<Database> {
    Database::open(vault)
}

pub fn create_note_simple_for_test(vault: &Path, name: &str) -> VaultResult<NoteMeta> {
    let clean = name.trim().trim_end_matches('/').trim_end_matches('\\');
    if clean.is_empty() {
        return Err(crate::error::VaultError::Other("empty".into()));
    }
    let stem = clean.trim_end_matches(".md");
    let filename = format!("{}.md", stem);

    // Pick inbox dir if exists
    let target_dir = if let Ok(entries) = std::fs::read_dir(vault) {
        entries
            .flatten()
            .find(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("0-") && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
            })
            .map(|e| e.path())
            .unwrap_or_else(|| vault.to_path_buf())
    } else {
        vault.to_path_buf()
    };

    let full_path = target_dir.join(&filename);
    let rel_path = full_path
        .strip_prefix(vault)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| filename.clone());

    if full_path.exists() {
        return Err(crate::error::VaultError::Other("exists".into()));
    }
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format!("# {}\n\n", stem);
    std::fs::write(&full_path, &content)?;

    let (fm, body) = parse_frontmatter(&content);
    let title = derive_title(&full_path, &fm, &body);
    let tags = collect_inline_tags_union(&body, &fm);
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    let hash = h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let modified = Utc::now().timestamp();
    let size = content.len() as u64;

    let db = Database::open(vault)?;
    db.upsert_note(&rel_path, &title, &body, &tags.join(" "), modified, size, &hash)?;
    let blocks = extract_blocks(&body, &rel_path);
    let raw_links = extract_links(&body, &blocks);
    db.replace_blocks(&blocks)?;
    db.insert_links(&rel_path, &raw_links)?;

    Ok(NoteMeta { path: rel_path, title, tags, modified, size })
}

pub fn list_notes_for_test(vault: &Path) -> VaultResult<Vec<NoteMeta>> {
    crate::commands::notes::list_notes_inner(vault, None)
}
