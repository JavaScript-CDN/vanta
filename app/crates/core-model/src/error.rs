use serde::Serialize;

/// Errors crossing the UI boundary. Never carries secrets or raw paths;
/// the UI maps `code` (+ `detail`) to a localised message.
#[derive(Debug, Clone, thiserror::Error, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum AppError {
    #[error("vault locked")] VaultLocked,
    #[error("vault already exists")] VaultExists,
    #[error("no vault")] NoVault,
    #[error("wrong password")] WrongPassword,
    #[error("too many attempts, retry in {0}s")] RetryLater(u64),
    #[error("not found: {0}")] NotFound(String),
    #[error("validation failed")] Validation(Vec<FieldIssue>),
    #[error("conflict: {0}")] Conflict(String),
    #[error("limit exceeded: {0}")] Limit(String),
    #[error("proxy error: {0}")] Proxy(String),
    #[error("launch error: {0}")] Launch(String),
    #[error("integrity check failed: {0}")] Integrity(String),
    #[error("io error")] Io,
    #[error("internal error")] Internal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldIssue { pub field: String, pub message_key: String }

impl FieldIssue {
    pub fn new(field: &str, key: &str) -> Self { Self { field: field.into(), message_key: key.into() } }
}

impl AppError {
    pub fn invalid(field: &str, key: &str) -> Self { Self::Validation(vec![FieldIssue::new(field, key)]) }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { tracing_free_log(&e); AppError::Io }
}

fn tracing_free_log(e: &dyn std::fmt::Display) { eprintln!("[vanta] io: {e}"); }

pub type CmdResult<T> = Result<T, AppError>;
