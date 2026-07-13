// Vault lifecycle commands: open, close, get info, pick folder

use crate::error::{VaultError, VaultResult};
use crate::state::{AppState, VaultInfo};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn open_vault(state: tauri::State<AppState>, path: String) -> VaultResult<VaultInfo> {
    state.open(PathBuf::from(path))
}

#[tauri::command]
pub fn close_vault(state: tauri::State<AppState>) {
    state.close();
}

#[tauri::command]
pub fn get_vault_info(state: tauri::State<AppState>) -> Option<VaultInfo> {
    state.info()
}

#[tauri::command]
pub async fn pick_vault(app: AppHandle) -> VaultResult<Option<String>> {
    // tauri-plugin-dialog 2.x: blocking_pick_folder on a tauri::async_runtime::spawn_blocking
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    app.dialog().file().pick_folder(move |folder| {
        let result = folder.and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().to_string());
        let _ = tx.send(result);
    });
    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(|e| VaultError::Other(format!("dialog join: {e}")))?;
    Ok(picked)
}
