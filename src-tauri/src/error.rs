// Unified error type for NoteVault backend
// Implements Serialize so it can cross the Tauri IPC boundary

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Walkdir error: {0}")]
    Walk(#[from] walkdir::Error),

    #[error("No vault is currently open")]
    NoVaultOpen,

    #[error("Vault path does not exist: {0}")]
    VaultNotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Note not found: {0}")]
    NoteNotFound(String),

    #[error("Operation failed: {0}")]
    Other(String),
}

pub type VaultResult<T> = Result<T, VaultError>;

// Tauri requires command errors to be Serialize
impl Serialize for VaultError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
