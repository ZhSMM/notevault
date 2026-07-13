// NoteVault - Local-first note app backend
// Entry: lib.rs -> run() called by main.rs

#[allow(hidden_glob_reexports)]
pub mod commands;
mod db;
mod error;
pub mod fsrs;
mod git;
mod state;

#[doc(hidden)]
pub mod test_helpers;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .setup(|app| {
            commands::ai::install_ai_state(&app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault::open_vault,
            commands::vault::close_vault,
            commands::vault::get_vault_info,
            commands::vault::pick_vault,
            commands::notes::list_notes,
            commands::notes::read_note,
            commands::notes::write_note,
            commands::notes::create_note,
            commands::notes::create_note_simple,
            commands::notes::reindex_vault,
            commands::notes::delete_note,
            commands::notes::rename_note,
            commands::notes::get_file_tree,
            commands::search::search,
            commands::links::get_backlinks,
            commands::links::get_forward_links,
            commands::links::get_dangling_links,
            commands::links::get_block,
            commands::links::get_blocks,
            commands::links::reindex_blocks,
            commands::cards::list_due_cards,
            commands::cards::get_card,
            commands::cards::review_card,
            commands::cards::count_due_cards,
            commands::cards::count_total_cards,
            commands::cards::card_stats,
            commands::cards::reindex_cards,
            commands::git::git_status,
            commands::git::git_init,
            commands::git::git_is_repo,
            commands::git::git_commit,
            commands::git::git_log,
            commands::links::get_graph_data,
            commands::ai::ai_list_models,
            commands::ai::ai_chat,
            commands::ai::ai_chat_stream,
            commands::ai::ai_cancel,
            commands::ai::ai_generate_cards,
            commands::publish::export_static,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NoteVault");
}
