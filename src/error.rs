use thiserror::Error;
use crate::domain::{storage::StorageError, config::ConfigError};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Server error: {0}")]
    Server(String),
}