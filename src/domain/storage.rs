use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;
use tokio::io::AsyncRead;
use tokio_stream::Stream;

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

    /// Store a data stream to storage at the given hash key.
    /// `content_length` must be the exact number of bytes the stream yields —
    /// object stores such as S3 require the size up front.
    /// Returns error if object already exists
    async fn store(
        &self,
        hash: &str,
        data: impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
        content_length: u64,
    ) -> Result<(), StorageError>;

    /// Retrieve object as a stream from storage
    /// Returns NotFound error if object doesn't exist
    async fn retrieve(&self, hash: &str)
        -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError>;
}
