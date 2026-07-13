// Link-related commands: backlinks, forward links, block lookup

use crate::db::BacklinkHit;
use crate::error::{VaultError, VaultResult};
use crate::state::AppState;

#[tauri::command]
pub fn get_backlinks(
    state: tauri::State<AppState>,
    note_path: String,
) -> VaultResult<Vec<BacklinkHit>> {
    state.with_db(|db| db.get_backlinks(&note_path))
}

#[tauri::command]
pub fn get_forward_links(
    state: tauri::State<AppState>,
    note_path: String,
) -> VaultResult<Vec<BacklinkHit>> {
    state.with_db(|db| db.get_forward_links(&note_path))
}

#[tauri::command]
pub fn get_dangling_links(
    state: tauri::State<AppState>,
) -> VaultResult<Vec<DanglingLink>> {
    let raw = state.with_db(|db| db.get_dangling_links())?;
    Ok(raw.into_iter().map(|(from, alias, ctx)| DanglingLink {
        from_note: from,
        to_alias: alias,
        context: ctx,
    }).collect())
}

#[derive(Debug, serde::Serialize)]
pub struct DanglingLink {
    pub from_note: String,
    pub to_alias: String,
    pub context: String,
}

#[tauri::command]
pub fn get_block(
    state: tauri::State<AppState>,
    block_id: String,
) -> VaultResult<Option<crate::commands::notes::Block>> {
    state.with_db(|db| db.get_block_by_id(&block_id))
}

#[tauri::command]
pub fn get_blocks(
    state: tauri::State<AppState>,
    note_path: String,
) -> VaultResult<Vec<crate::commands::notes::Block>> {
    state.with_db(|db| db.get_blocks_for_note(&note_path))
}

#[tauri::command]
pub fn reindex_blocks(
    state: tauri::State<AppState>,
) -> VaultResult<ReindexBlocksResult> {
    use crate::commands::notes::{extract_blocks, extract_links, list_notes_inner};
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let notes = list_notes_inner(&base, None)?;
    let mut notes_indexed = 0;
    let mut blocks_total = 0;
    let mut links_total = 0;
    for n in &notes {
        let full = base.join(&n.path);
        let raw = std::fs::read_to_string(&full).unwrap_or_default();
        let (_fm, body) = crate::commands::notes::parse_frontmatter(&raw);
        let blocks = extract_blocks(&body, &n.path);
        let links = extract_links(&body, &blocks);
        state.with_db(|db| {
            db.replace_blocks(&blocks)?;
            db.insert_links(&n.path, &links)?;
            Ok::<(), VaultError>(())
        })?;
        notes_indexed += 1;
        blocks_total += blocks.len();
        links_total += links.len();
    }
    Ok(ReindexBlocksResult { notes_indexed, blocks_total, links_total })
}

#[derive(Debug, serde::Serialize)]
pub struct ReindexBlocksResult {
    pub notes_indexed: usize,
    pub blocks_total: usize,
    pub links_total: usize,
}

use std::path::PathBuf;

#[derive(Debug, serde::Serialize)]
pub struct GraphNodeData {
    pub id: String,
    pub label: String,
    pub kind: String,            // "note"
    pub size: i64,               // degree + 5
    pub in_degree: i64,
    pub out_degree: i64,
    pub tags: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphEdgeData {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,            // "wiki" | "block_ref" | "transclusion"
}

#[derive(Debug, serde::Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNodeData>,
    pub edges: Vec<GraphEdgeData>,
}

#[tauri::command]
pub fn get_graph_data(state: tauri::State<AppState>) -> VaultResult<GraphData> {
    state.with_db(|db| {
        // All notes
        let mut note_stmt = db.conn.prepare(
            "SELECT path, title FROM notes"
        )?;
        let note_rows = note_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut path_to_title: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for r in note_rows {
            let (p, t) = r?;
            path_to_title.insert(p, t);
        }
        // All tags per note
        let mut note_tags: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        {
            let mut tag_stmt = db.conn.prepare(
                "SELECT note_path, tag FROM tags"
            )?;
            let tag_rows = tag_stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in tag_rows {
                let (p, t) = r?;
                note_tags.entry(p).or_default().push(t);
            }
        }

        // All resolved links (to_note_id is not null)
        let mut link_stmt = db.conn.prepare(
            "SELECT from_note_id, to_note_id, link_type, from_block_id, to_block_id
             FROM links WHERE to_note_id IS NOT NULL"
        )?;
        let link_rows = link_stmt.query_map([], |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        )))?;
        let mut edges = Vec::new();
        let mut in_degree: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let mut out_degree: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for r in link_rows {
            let (from, to, kind, from_block, to_block) = r?;
            // For block-level refs, make the edge a block-edge so the graph
            // doesn't get too dense. MVP: only count note-to-note edges.
            edges.push(GraphEdgeData {
                id: format!("{}->{}/{}/{}", from, to, from_block.as_deref().unwrap_or("-"), to_block.as_deref().unwrap_or("-")),
                source: from.clone(),
                target: to.clone(),
                kind,
            });
            *out_degree.entry(from).or_insert(0) += 1;
            *in_degree.entry(to).or_insert(0) += 1;
        }

        let mut nodes: Vec<GraphNodeData> = path_to_title.iter().map(|(p, t)| {
            let indeg = *in_degree.get(p).unwrap_or(&0);
            let outdeg = *out_degree.get(p).unwrap_or(&0);
            let size = indeg + outdeg + 3;
            GraphNodeData {
                id: p.clone(),
                label: t.clone(),
                kind: "note".into(),
                size,
                in_degree: indeg,
                out_degree: outdeg,
                tags: note_tags.get(p).cloned().unwrap_or_default(),
            }
        }).collect();
        // Sort: high-degree first
        nodes.sort_by(|a, b| (b.in_degree + b.out_degree).cmp(&(a.in_degree + a.out_degree)));

        Ok(GraphData { nodes, edges })
    })
}
