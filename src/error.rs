#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("malformed manifest: {0}")]
    Manifest(String),
    #[error("unsafe artifact path: {0}")]
    UnsafePath(String),
}
