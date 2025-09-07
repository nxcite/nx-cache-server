use async_trait::async_trait;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::{config::Region, Client, Config as S3Config};
use clap::Parser;
use tokio::io::AsyncRead;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

use crate::domain::{
    config::{ConfigError, ConfigValidator},
    storage::{StorageError, StorageProvider},
};

#[derive(Parser, Debug, Clone)]
pub struct AwsStorageConfig {
    #[arg(long, env = "AWS_REGION")]
    pub region: String,

    #[arg(long, env = "AWS_ACCESS_KEY_ID")]
    pub access_key_id: String,

    #[arg(long, env = "AWS_SECRET_ACCESS_KEY")]
    pub secret_access_key: String,

    #[arg(long, env = "S3_BUCKET_NAME")]
    pub bucket_name: String,

    #[arg(long, env = "S3_ENDPOINT_URL")]
    pub endpoint_url: String,

    #[arg(long, env = "S3_TIMEOUT", default_value = "30")]
    pub timeout_seconds: u64,
}

impl ConfigValidator for AwsStorageConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.region.is_empty() {
            return Err(ConfigError::MissingField("AWS region"));
        }
        if self.access_key_id.is_empty() {
            return Err(ConfigError::MissingField("AWS access key ID"));
        }
        if self.secret_access_key.is_empty() {
            return Err(ConfigError::MissingField("AWS secret access key"));
        }
        if self.bucket_name.is_empty() {
            return Err(ConfigError::MissingField("S3 bucket name"));
        }
        if !self.endpoint_url.starts_with("http://") && !self.endpoint_url.starts_with("https://") {
            return Err(ConfigError::Invalid(
                "S3 endpoint URL must start with http:// or https://",
            ));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct S3Storage {
    client: Client,
    bucket_name: String,
}

impl S3Storage {
    pub async fn new(config: &AwsStorageConfig) -> Result<Self, StorageError> {
        let s3_config = S3Config::builder()
            .region(Region::new(config.region.clone()))
            .endpoint_url(&config.endpoint_url)
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                &config.access_key_id,
                &config.secret_access_key,
                None,
                None,
                "nx-cache-server",
            ))
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(std::time::Duration::from_secs(config.timeout_seconds))
                    .build(),
            )
            .build();

        let client = Client::from_conf(s3_config);

        Ok(Self {
            client,
            bucket_name: config.bucket_name.clone(),
        })
    }
}

#[async_trait]
impl StorageProvider for S3Storage {
    async fn exists(&self, hash: &str) -> Result<bool, StorageError> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket_name)
            .key(hash)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) if e.to_string().contains("NotFound") => Ok(false),
            Err(e) => {
                tracing::error!("S3 head_object failed: {:?}", e);
                Err(StorageError::OperationFailed)
            }
        }
    }

    async fn store(
        &self,
        hash: &str,
        mut data: ReaderStream<impl AsyncRead + Send + Unpin>,
    ) -> Result<(), StorageError> {
        if self.exists(hash).await? {
            return Err(StorageError::AlreadyExists);
        }

        // For simplicity, read all data into memory first
        // TODO: Implement true streaming for better memory efficiency
        let mut buffer = Vec::new();
        while let Some(chunk) = data.next().await {
            let chunk = chunk.map_err(|_| StorageError::OperationFailed)?;
            buffer.extend_from_slice(&chunk);
        }

        let body = aws_sdk_s3::primitives::ByteStream::from(buffer);

        self.client
            .put_object()
            .bucket(&self.bucket_name)
            .key(hash)
            .body(body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("S3 put_object failed: {:?}", e);
                StorageError::OperationFailed
            })?;

        Ok(())
    }

    async fn retrieve(
        &self,
        hash: &str,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
        let result = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(hash)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("S3 get_object failed: {:?}", e);
                if e.to_string().contains("NoSuchKey") {
                    StorageError::NotFound
                } else {
                    StorageError::OperationFailed
                }
            })?;

        // Direct streaming - no buffering
        Ok(Box::new(result.body.into_async_read()))
    }
}
