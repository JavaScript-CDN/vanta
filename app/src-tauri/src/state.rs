//! Application state: the vault (locked/unlocked), the DB handle (only while unlocked),
//! the launch supervisor, and the running forwarders.

use core_db::Db;
use core_fp::{EngineVersion, HostProfile};
use core_launch::Supervisor;
use core_model::{AppError, CmdResult};
use core_vault::Vault;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::async_runtime::RwLock;

pub struct AppState {
    pub data_root: PathBuf,
    pub vault: RwLock<Vault>,
    pub db: RwLock<Option<Db>>,
    pub supervisor: Supervisor,
    pub forwarders: Mutex<HashMap<String, core_launch::Forwarder>>,
    pub host: HostProfile,
    pub engine: EngineVersion,
}

impl AppState {
    pub fn new(data_root: PathBuf, host: HostProfile, engine: EngineVersion) -> (Self, tokio::sync::mpsc::UnboundedReceiver<core_launch::ExitEvent>) {
        let (supervisor, rx) = Supervisor::new();
        let vault = Vault::new(data_root.join("vault"));
        (Self {
            data_root, vault: RwLock::new(vault), db: RwLock::new(None),
            supervisor, forwarders: Mutex::new(HashMap::new()), host, engine,
        }, rx)
    }

    /// Run a closure against the open DB, erroring if the vault is locked.
    pub async fn with_db<T>(&self, f: impl FnOnce(&Db) -> CmdResult<T>) -> CmdResult<T> {
        let guard = self.db.read().await;
        let db = guard.as_ref().ok_or(AppError::VaultLocked)?;
        f(db)
    }
}
