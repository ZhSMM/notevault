// SQLite database wrapper for NoteVault
// - Stores notes metadata
// - Provides FTS5 full-text search
// - Index rebuilt on vault open

use crate::error::{VaultError, VaultResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub struct Database {
    pub conn: Connection,
    #[allow(dead_code)]
    path: PathBuf,
}

const SCHEMA_SQL: &str = r#"
-- Notes metadata
CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    modified INTEGER NOT NULL,  -- unix timestamp seconds
    size INTEGER NOT NULL,
    hash TEXT NOT NULL          -- sha256 of file content, for change detection
);

CREATE INDEX IF NOT EXISTS idx_notes_modified ON notes(modified DESC);
CREATE INDEX IF NOT EXISTS idx_notes_title ON notes(title);

-- FTS5 virtual table for full-text search (contentless)
-- We use a rowid that mirrors notes.id
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    body,
    tags,
    content='',
    tokenize='unicode61 remove_diacritics 2'
);

-- Blocks: each significant block in a note (heading, paragraph, code, list)
-- Block IDs are content-hash based, so they're stable as long as content
-- doesn't change. ID format: "blk_<8 hex chars>".
CREATE TABLE IF NOT EXISTS blocks (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    type TEXT NOT NULL,         -- heading | paragraph | code | list | blockquote
    level INTEGER,              -- 1-6 for headings
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    FOREIGN KEY (note_id) REFERENCES notes(path) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_blocks_note ON blocks(note_id);
CREATE INDEX IF NOT EXISTS idx_blocks_order ON blocks(note_id, order_index);

-- Links: any link from one note to another (or to a block within a note)
-- Includes wikilinks, block refs, and transclusions.
CREATE TABLE IF NOT EXISTS links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_note_id TEXT NOT NULL,
    from_block_id TEXT,         -- nullable
    to_note_id TEXT,            -- resolved target, NULL if dangling
    to_block_id TEXT,           -- for ((id)) and [[note#^id]] references
    to_alias TEXT NOT NULL,     -- raw target as written
    link_type TEXT NOT NULL,    -- wiki | block_ref | transclusion
    context TEXT,               -- short surrounding text
    created_at INTEGER NOT NULL,
    UNIQUE (from_note_id, from_block_id, to_alias, link_type)
);
CREATE INDEX IF NOT EXISTS idx_links_from ON links(from_note_id);
CREATE INDEX IF NOT EXISTS idx_links_to ON links(to_note_id);
CREATE INDEX IF NOT EXISTS idx_links_to_block ON links(to_block_id);

-- Cards: flashcard with FSRS state
-- A card is a list item with #card / #card-reverse / #cloze marker
CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY,
    note_path TEXT NOT NULL,
    block_id TEXT,                 -- source block
    card_type TEXT NOT NULL,       -- basic | reverse | cloze
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    cloze_text TEXT,               -- for cloze: the source with markers stripped
    line_index INTEGER NOT NULL,   -- line number in note (for stable ID)
    -- FSRS state
    difficulty REAL NOT NULL DEFAULT 5.0,
    stability REAL NOT NULL DEFAULT 2.0,
    interval_days REAL NOT NULL DEFAULT 0.0,
    reps INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    last_review_at INTEGER,
    next_review_at INTEGER NOT NULL,    -- unix timestamp
    state TEXT NOT NULL DEFAULT 'new',  -- new | learning | review | relearning
    created_at INTEGER NOT NULL,
    FOREIGN KEY (note_path) REFERENCES notes(path) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_cards_due ON cards(next_review_at);
CREATE INDEX IF NOT EXISTS idx_cards_note ON cards(note_path);
CREATE INDEX IF NOT EXISTS idx_cards_state ON cards(state);
"#;

impl Database {
    pub fn open(vault_path: &Path) -> VaultResult<Self> {
        let db_path = vault_path.join(".notevault").join("index.sqlite");
        std::fs::create_dir_all(db_path.parent().unwrap())?;

        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(SCHEMA_SQL)?;

        Ok(Self {
            conn,
            path: db_path,
        })
    }

    /// Insert or update a note in the metadata table and FTS index.
    /// Strategy: delete the old FTS row (if exists), then insert fresh.
    pub fn upsert_note(
        &self,
        path: &str,
        title: &str,
        body: &str,
        tags: &str,
        modified: i64,
        size: u64,
        hash: &str,
    ) -> VaultResult<()> {
        let tx = self.conn.unchecked_transaction()?;

        // Get existing rowid, if any
        let existing_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM notes WHERE path = ?1",
                params![path],
                |r| r.get(0),
            )
            .optional()?;

        let id: i64 = if let Some(eid) = existing_id {
            // Update metadata
            tx.execute(
                "UPDATE notes SET title=?1, modified=?2, size=?3, hash=?4 WHERE id=?5",
                params![title, modified, size, hash, eid],
            )?;
            // Delete old FTS row
            tx.execute(
                "INSERT INTO notes_fts(notes_fts, rowid, title, body, tags)
                 VALUES('delete', ?1, '', '', '')",
                params![eid],
            )?;
            eid
        } else {
            tx.execute(
                "INSERT INTO notes (path, title, modified, size, hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![path, title, modified, size, hash],
            )?;
            tx.last_insert_rowid()
        };

        // Insert new FTS row
        tx.execute(
            "INSERT INTO notes_fts(rowid, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
            params![id, title, body, tags],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_note(&self, path: &str) -> VaultResult<()> {
        let rowid: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM notes WHERE path = ?1",
                params![path],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(rid) = rowid {
            self.conn.execute(
                "INSERT INTO notes_fts(notes_fts, rowid, title, body, tags)
                 VALUES('delete', ?1, '', '', '')",
                params![rid],
            )?;
            self.conn.execute(
                "DELETE FROM notes WHERE id = ?1",
                params![rid],
            )?;
            // ON DELETE CASCADE cleans up blocks; clean up links manually
            self.conn.execute("DELETE FROM links WHERE from_note_id = ?1", params![path])?;
        }
        Ok(())
    }

    // ---------- Blocks ----------

    /// Replace all blocks for a note (delete old, insert new).
    pub fn replace_blocks(&self, blocks: &[crate::commands::notes::Block]) -> VaultResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        if let Some(first) = blocks.first() {
            tx.execute("DELETE FROM blocks WHERE note_id = ?1", params![first.note_id])?;
            tx.execute("DELETE FROM links WHERE from_note_id = ?1", params![first.note_id])?;
        }
        for b in blocks {
            tx.execute(
                "INSERT INTO blocks (id, note_id, type, level, content, content_hash, order_index)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![b.id, b.note_id, b.r#type, b.level, b.content, b.content_hash, b.order_index],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_blocks_for_note(&self, note_id: &str) -> VaultResult<Vec<crate::commands::notes::Block>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, note_id, type, level, content, content_hash, order_index
             FROM blocks WHERE note_id = ?1 ORDER BY order_index"
        )?;
        let rows = stmt.query_map(params![note_id], |r| {
            Ok(crate::commands::notes::Block {
                id: r.get(0)?,
                note_id: r.get(1)?,
                r#type: r.get(2)?,
                level: r.get::<_, Option<u8>>(3)?,
                content: r.get(4)?,
                content_hash: r.get(5)?,
                order_index: r.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    // ---------- Links ----------

    /// Insert raw links for a note, resolving to_alias → to_note_id / to_block_id.
    /// Resolution rules:
    ///   - "wiki" links: to_alias = note title. Try matching by path (alias.md)
    ///     first, then by title. If a block_id is provided, also resolve the
    ///     target block by scanning notes for it.
    ///   - "block_ref" links: to_alias is a block id. Find the note containing
    ///     a block with that id.
    ///   - "transclusion": same as wiki.
    pub fn insert_links(
        &self,
        from_note_id: &str,
        links: &[crate::commands::notes::RawLink],
    ) -> VaultResult<()> {
        // Build a lookup: title -> path
        let mut title_to_path: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT path, title FROM notes")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in rows {
                let (p, t) = r?;
                title_to_path.insert(t, p.clone());
                title_to_path.insert(p.trim_end_matches(".md").to_string(), p);
            }
        }
        // Build a lookup: block id -> (note path)
        let mut block_to_note: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT id, note_id FROM blocks")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in rows {
                let (b, n) = r?;
                block_to_note.insert(b, n);
            }
        }

        let tx = self.conn.unchecked_transaction()?;
        let now = chrono::Utc::now().timestamp();
        for link in links {
            let to_note = title_to_path.get(&link.to_alias).cloned();
            let to_block_note = block_to_note.get(&link.to_alias).cloned();
            // Final to_note_id: explicit block ref wins, else wiki resolution
            let resolved_to_note = if link.link_type == "block_ref" {
                to_block_note.or(to_note)
            } else {
                to_note
            };
            let resolved_to_block = if link.link_type == "block_ref" {
                Some(link.to_alias.clone())
            } else {
                link.to_block_id.clone()
            };
            tx.execute(
                "INSERT OR REPLACE INTO links
                 (from_note_id, from_block_id, to_note_id, to_block_id, to_alias, link_type, context, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    from_note_id,
                    link.from_block_id,
                    resolved_to_note,
                    resolved_to_block,
                    link.to_alias,
                    link.link_type,
                    link.context,
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// All notes that link to the given target (reverse links).
    pub fn get_backlinks(&self, target_note_id: &str) -> VaultResult<Vec<BacklinkHit>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.from_note_id, n.title, l.from_block_id, l.to_block_id, l.context, l.link_type
             FROM links l
             LEFT JOIN notes n ON n.path = l.from_note_id
             WHERE l.to_note_id = ?1
             ORDER BY n.path, l.created_at DESC"
        )?;
        let rows = stmt.query_map(params![target_note_id], |r| {
            Ok(BacklinkHit {
                source_path: r.get(0)?,
                source_title: r.get(1)?,
                from_block_id: r.get(2)?,
                to_block_id: r.get(3)?,
                context: r.get(4)?,
                link_type: r.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    /// All notes the given source links to.
    pub fn get_forward_links(&self, source_note_id: &str) -> VaultResult<Vec<BacklinkHit>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.to_note_id, n.title, l.from_block_id, l.to_block_id, l.context, l.link_type
             FROM links l
             LEFT JOIN notes n ON n.path = l.to_note_id
             WHERE l.from_note_id = ?1 AND l.to_note_id IS NOT NULL
             ORDER BY l.created_at DESC"
        )?;
        let rows = stmt.query_map(params![source_note_id], |r| {
            Ok(BacklinkHit {
                source_path: r.get(0)?,
                source_title: r.get(1)?,
                from_block_id: r.get(2)?,
                to_block_id: r.get(3)?,
                context: r.get(4)?,
                link_type: r.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    /// All unresolved (dangling) wikilinks across the vault.
    pub fn get_dangling_links(&self) -> VaultResult<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.from_note_id, l.to_alias, l.context
             FROM links l
             WHERE l.to_note_id IS NULL
             ORDER BY l.from_note_id"
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    /// Block content by id (used for block-ref rendering)
    pub fn get_block_by_id(&self, block_id: &str) -> VaultResult<Option<crate::commands::notes::Block>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, note_id, type, level, content, content_hash, order_index
             FROM blocks WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![block_id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(crate::commands::notes::Block {
                id: r.get(0)?,
                note_id: r.get(1)?,
                r#type: r.get(2)?,
                level: r.get::<_, Option<u8>>(3)?,
                content: r.get(4)?,
                content_hash: r.get(5)?,
                order_index: r.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    // ---------- Cards (FSRS) ----------

    /// Replace cards for a note (delete old, insert new) — same as blocks/links.
    pub fn replace_cards(&self, cards: &[crate::commands::notes::Card]) -> VaultResult<()> {
        let tx = self.conn.unchecked_transaction()?;
        if let Some(first) = cards.first() {
            tx.execute("DELETE FROM cards WHERE note_path = ?1", params![first.note_path])?;
        }
        for c in cards {
            tx.execute(
                "INSERT OR REPLACE INTO cards
                 (id, note_path, block_id, card_type, question, answer, cloze_text, line_index,
                  difficulty, stability, interval_days, reps, lapses, last_review_at,
                  next_review_at, state, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    c.id, c.note_path, c.block_id, c.card_type, c.question, c.answer, c.cloze_text,
                    c.line_index, c.difficulty, c.stability, c.interval_days, c.reps, c.lapses,
                    c.last_review_at, c.next_review_at, c.state, c.created_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_due_cards(&self, now: i64, limit: u32) -> VaultResult<Vec<crate::commands::notes::Card>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, note_path, block_id, card_type, question, answer, cloze_text, line_index,
                    difficulty, stability, interval_days, reps, lapses, last_review_at,
                    next_review_at, state, created_at
             FROM cards
             WHERE next_review_at <= ?1 OR state = 'new'
             ORDER BY next_review_at ASC
             LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![now, limit], Self::card_from_row)?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    pub fn count_due_cards(&self, now: i64) -> VaultResult<i64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE next_review_at <= ?1 OR state = 'new'",
            params![now],
            |r| r.get(0),
        )?;
        Ok(n)
    }

    pub fn count_cards(&self) -> VaultResult<i64> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM cards", [], |r| r.get(0))?;
        Ok(n)
    }

    pub fn get_card(&self, id: &str) -> VaultResult<Option<crate::commands::notes::Card>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, note_path, block_id, card_type, question, answer, cloze_text, line_index,
                    difficulty, stability, interval_days, reps, lapses, last_review_at,
                    next_review_at, state, created_at
             FROM cards WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(Self::card_from_row(r)?))
        } else {
            Ok(None)
        }
    }

    pub fn save_card(&self, c: &crate::commands::notes::Card) -> VaultResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO cards
             (id, note_path, block_id, card_type, question, answer, cloze_text, line_index,
              difficulty, stability, interval_days, reps, lapses, last_review_at,
              next_review_at, state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                c.id, c.note_path, c.block_id, c.card_type, c.question, c.answer, c.cloze_text,
                c.line_index, c.difficulty, c.stability, c.interval_days, c.reps, c.lapses,
                c.last_review_at, c.next_review_at, c.state, c.created_at
            ],
        )?;
        Ok(())
    }

    fn card_from_row(r: &rusqlite::Row) -> rusqlite::Result<crate::commands::notes::Card> {
        Ok(crate::commands::notes::Card {
            id: r.get(0)?,
            note_path: r.get(1)?,
            block_id: r.get(2)?,
            card_type: r.get(3)?,
            question: r.get(4)?,
            answer: r.get(5)?,
            cloze_text: r.get(6)?,
            line_index: r.get(7)?,
            difficulty: r.get(8)?,
            stability: r.get(9)?,
            interval_days: r.get(10)?,
            reps: r.get(11)?,
            lapses: r.get(12)?,
            last_review_at: r.get(13)?,
            next_review_at: r.get(14)?,
            state: r.get(15)?,
            created_at: r.get(16)?,
        })
    }

    /// Sync the FTS index with the file system. Adds new files, removes deleted ones.
    /// Each entry: (path, title, body, tags_joined, modified, size, hash)
    /// Returns the number of changes (added, removed).
    pub fn sync_with_files(
        &self,
        current_files: &[(String, String, String, String, i64, u64, String)],
    ) -> VaultResult<(usize, usize)> {
        let mut indexed_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let mut stmt = self.conn.prepare("SELECT path FROM notes")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows {
                if let Ok(p) = r {
                    indexed_paths.insert(p);
                }
            }
        }

        let on_disk: std::collections::HashSet<String> = current_files
            .iter()
            .map(|(p, ..)| p.clone())
            .collect();

        // Remove from index: files that are indexed but no longer on disk
        let mut removed = 0;
        let to_remove: Vec<String> = indexed_paths.difference(&on_disk).cloned().collect();
        for path in to_remove {
            self.remove_note(&path)?;
            removed += 1;
        }

        // Add to index: files on disk that aren't indexed (or whose hash differs)
        let mut added = 0;
        for (path, title, body, tags, modified, size, hash) in current_files {
            let needs_insert = match self
                .conn
                .query_row(
                    "SELECT hash FROM notes WHERE path = ?1",
                    params![path],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
            {
                Some(existing_hash) => existing_hash != *hash,
                None => true,
            };
            if needs_insert {
                self.upsert_note(path, title, body, tags, *modified, *size, hash)?;
                added += 1;
            }
        }
        Ok((added, removed))
    }

    pub fn search(&self, query: &str, limit: u32) -> VaultResult<Vec<SearchHit>> {
        let escaped = escape_fts_query(query);
        if escaped.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT n.path, n.title,
                    COALESCE(snippet(notes_fts, 1, '<mark>', '</mark>', '...', 32), '') as snip,
                    COALESCE(bm25(notes_fts), 0.0) as rank
             FROM notes_fts
             JOIN notes n ON n.id = notes_fts.rowid
             WHERE notes_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![escaped, limit], |row| {
            Ok(SearchHit {
                path: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
                rank: row.get(3)?,
            })
        })?;

        let mut hits = Vec::new();
        for r in rows {
            hits.push(r?);
        }
        Ok(hits)
    }

    pub fn count_notes(&self) -> VaultResult<i64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?;
        Ok(n)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub rank: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct BacklinkHit {
    pub source_path: String,
    pub source_title: String,
    pub from_block_id: Option<String>,
    pub to_block_id: Option<String>,
    pub context: String,
    pub link_type: String,
}

/// Build a safe FTS5 prefix query. Each whitespace-separated token gets
/// quoted and a `*` appended for prefix matching, so partial CJK input
/// still works (unicode61 tokenizes on word boundaries).
fn escape_fts_query(q: &str) -> String {
    q.split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| {
            // Strip non-word chars except Chinese characters and CJK basic range
            let cleaned: String = t
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || is_cjk(*c))
                .collect();
            if cleaned.is_empty() {
                String::new()
            } else {
                format!("\"{}\"*", cleaned)
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'  // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
    )
}

#[allow(dead_code)]
fn _err_silencer(e: VaultError) -> VaultError {
    e
}
