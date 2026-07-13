// Search command - delegates to Database::search which uses FTS5

use crate::db::SearchHit;
use crate::error::VaultResult;
use crate::state::AppState;

#[tauri::command]
pub fn search(
    state: tauri::State<AppState>,
    query: String,
    limit: Option<u32>,
) -> VaultResult<Vec<SearchHit>> {
    let lim = limit.unwrap_or(20);
    state.with_db(|db| db.search(&query, lim))
}
