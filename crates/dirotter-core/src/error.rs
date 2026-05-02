use thiserror::Error;

#[derive(Error, Debug)]
pub enum DirOtterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Node not found: {id:?}")]
    NodeNotFound { id: crate::NodeId },

    #[error("Invalid path: {path}")]
    InvalidPath { path: String },

    #[error("Scan error: {0}")]
    Scan(String),

    #[error("Cache error: {0}")]
    Cache(String),
}

pub type Result<T> = std::result::Result<T, DirOtterError>;
