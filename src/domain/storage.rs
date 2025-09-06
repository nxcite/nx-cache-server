use async_trait::async_trait;
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Object not found")]
    NotFound,
    #[error("Object already exists")]
    AlreadyExists,
    #[error("Storage operation failed")]
    OperationFailed,
}

#[async_trait]
pub trait StorageProvider: Send + Sync + 'static {
    /// Check if an object exists at the given hash key
    async fn exists(&self, hash: &str) -> Result<bool, StorageError>;
    
    /// Store data stream to storage at the given hash key
    /// Returns error if object already exists
    async fn store(
        &self, 
        hash: &str, 
        data: ReaderStream<impl AsyncRead + Send + Unpin>
    ) -> Result<(), StorageError>;
    
    /// Retrieve object as a stream from storage
    /// Returns NotFound error if object doesn't exist
    async fn retrieve(&self, hash: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError>;
}