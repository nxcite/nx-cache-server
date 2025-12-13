use async_trait::async_trait;
use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::future::ProvideRegion as ProvideRegionFuture;
use aws_config::meta::region::{ProvideRegion, RegionProviderChain};
use aws_credential_types::provider::future::ProvideCredentials as ProvideCredentialsFuture;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::{Credentials, ProvideCredentials};
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
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
    pub region: Option<String>,

    #[arg(long, env = "AWS_ACCESS_KEY_ID")]
    pub access_key_id: Option<String>,

    #[arg(long, env = "AWS_SECRET_ACCESS_KEY")]
    pub secret_access_key: Option<String>,

    #[arg(long, env = "AWS_SESSION_TOKEN")]
    pub session_token: Option<String>,

    #[arg(long, env = "S3_BUCKET_NAME")]
    pub bucket_name: String,

    #[arg(long, env = "S3_ENDPOINT_URL")]
    pub endpoint_url: Option<String>,

    #[arg(long, env = "S3_TIMEOUT", default_value = "30")]
    pub timeout_seconds: u64,
}

impl ProvideRegion for AwsStorageConfig {
    fn region(&self) -> ProvideRegionFuture<'_> {
        let region = self.region.clone();
        ProvideRegionFuture::new(async {
            RegionProviderChain::first_try(region.map(Region::new))
                .or_default_provider()
                .region()
                .await
        })
    }
}

impl ProvideCredentials for AwsStorageConfig {
    fn provide_credentials<'a>(&'a self) -> ProvideCredentialsFuture<'a>
    where
        Self: 'a,
    {
        match (self.access_key_id.as_ref(), self.secret_access_key.as_ref()) {
            (Some(access_key_id), Some(secret_access_key)) => {
                ProvideCredentialsFuture::ready(Ok(Credentials::new(
                    access_key_id,
                    secret_access_key,
                    self.session_token.clone(),
                    None,
                    "nx-cache-server",
                )))
            }
            _ => ProvideCredentialsFuture::new(async {
                DefaultCredentialsChain::builder()
                    .region(self.clone())
                    .build()
                    .await
                    .provide_credentials()
                    .await
            }),
        }
    }
}

impl ConfigValidator for AwsStorageConfig {
    async fn validate(&self) -> Result<(), ConfigError> {
        if self.bucket_name.is_empty() {
            return Err(ConfigError::MissingField("S3 bucket name"));
        }
        if let Some(endpoint_url) = &self.endpoint_url {
            if !endpoint_url.starts_with("http://") && !endpoint_url.starts_with("https://") {
                return Err(ConfigError::Invalid(
                    "S3 endpoint URL must start with http:// or https://",
                ));
            }
        }
        match (self.access_key_id.as_ref(), self.secret_access_key.as_ref()) {
            (Some(..), None) => return Err(ConfigError::MissingField("AWS_ACCESS_KEY_ID")),
            (None, Some(..)) => return Err(ConfigError::MissingField("AWS_SECRET_ACCESS_KEY")),
            _ => {}
        }
        if self.region().await.is_none() {
            return Err(ConfigError::MissingField("AWS_REGION"));
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
        let mut s3_config_builder = S3Config::builder()
            .behavior_version_latest()
            .region(config.region().await)
            .credentials_provider(config.clone())
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(std::time::Duration::from_secs(config.timeout_seconds))
                    .build(),
            );

        // Only set custom endpoint if provided, otherwise use AWS default
        if let Some(endpoint_url) = &config.endpoint_url {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint_url);
        }

        let s3_config = s3_config_builder.build();

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
            Err(e) => match e.into_service_error() {
                HeadObjectError::NotFound(_) => Ok(false),
                other => {
                    tracing::error!("S3 head_object failed: {:?}", other);
                    Err(StorageError::OperationFailed)
                }
            },
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
            .map_err(|e| match e.into_service_error() {
                GetObjectError::NoSuchKey(_) => StorageError::NotFound,
                other => {
                    tracing::error!("S3 get_object failed: {:?}", other);
                    StorageError::OperationFailed
                }
            })?;

        // Direct streaming - no buffering
        Ok(Box::new(result.body.into_async_read()))
    }
}
