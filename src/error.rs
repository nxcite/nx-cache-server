use crate::domain::{config::ConfigError, storage::StorageError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Server error: {0}")]
    Server(String),
}
