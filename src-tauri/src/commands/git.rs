// Tauri commands for Git integration (via shell `git` command)

use crate::error::{VaultError, VaultResult};
use crate::git::{self, GitLogEntry, GitStatus};
use crate::state::AppState;
use std::path::Path;

#[tauri::command]
pub fn git_status(state: tauri::State<AppState>) -> VaultResult<GitStatus> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    git::git_status(&Path::new(&info.path))
}

#[tauri::command]
pub fn git_init(state: tauri::State<AppState>) -> VaultResult<GitStatus> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    git::git_init(&Path::new(&info.path))
}

#[tauri::command]
pub fn git_is_repo(state: tauri::State<AppState>) -> VaultResult<bool> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    Ok(git::is_git_repo(&Path::new(&info.path)))
}

#[tauri::command]
pub fn git_commit(state: tauri::State<AppState>, message: String) -> VaultResult<GitLogEntry> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    git::git_commit(&Path::new(&info.path), &message)
}

#[tauri::command]
pub fn git_log(state: tauri::State<AppState>, limit: Option<usize>) -> VaultResult<Vec<GitLogEntry>> {
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    git::git_log(&Path::new(&info.path), limit.unwrap_or(20))
}
