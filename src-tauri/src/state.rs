// Application state - holds the active vault and its SQLite index
// Wrapped in Mutex; commands acquire locks for the duration of their work.

use crate::db::Database;
use crate::error::{VaultError, VaultResult};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex as StdMutex;

#[derive(Debug, Clone, Serialize)]
pub struct VaultInfo {
    pub path: String,
    pub name: String,
}

pub struct AppState {
    inner: StdMutex<Option<ActiveVault>>,
}

struct ActiveVault {
    info: VaultInfo,
    db: Database,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: StdMutex::new(None),
        }
    }

    pub fn open(&self, path: PathBuf) -> VaultResult<VaultInfo> {
        if !path.exists() {
            return Err(VaultError::VaultNotFound(
                path.to_string_lossy().to_string(),
            ));
        }
        if !path.is_dir() {
            return Err(VaultError::InvalidPath(
                path.to_string_lossy().to_string(),
            ));
        }

        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "vault".to_string());

        let db = Database::open(&path)?;
        let info = VaultInfo {
            path: path.to_string_lossy().to_string(),
            name,
        };

        let mut guard = self.inner.lock().unwrap();
        *guard = Some(ActiveVault { info: info.clone(), db });
        Ok(info)
    }

    pub fn close(&self) {
        let mut guard = self.inner.lock().unwrap();
        *guard = None;
    }

    pub fn info(&self) -> Option<VaultInfo> {
        self.inner
            .lock()
            .unwrap()
            .as_ref()
            .map(|v| v.info.clone())
    }

    pub fn with_db<F, R>(&self, f: F) -> VaultResult<R>
    where
        F: FnOnce(&Database) -> VaultResult<R>,
    {
        let guard = self.inner.lock().unwrap();
        let vault = guard.as_ref().ok_or(VaultError::NoVaultOpen)?;
        f(&vault.db)
    }
}
