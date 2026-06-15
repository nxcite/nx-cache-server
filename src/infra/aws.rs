use async_trait::async_trait;
use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::environment::region::EnvironmentVariableRegionProvider;
use aws_config::imds::region::ImdsRegionProvider;
use aws_config::meta::region::future::ProvideRegion as ProvideRegionFuture;
use aws_config::meta::region::{ProvideRegion, RegionProviderChain};
use aws_config::profile::region::ProfileFileRegionProvider;
use aws_config::provider_config::ProviderConfig;
use aws_credential_types::provider::future::ProvideCredentials as ProvideCredentialsFuture;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::SharedHttpClient;
use aws_sdk_s3::config::{Credentials, ProvideCredentials};
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use aws_sdk_s3::{config::Region, Client, Config as S3Config};
use aws_smithy_http_client::tls::rustls_provider::CryptoMode;
use aws_smithy_http_client::{tls, Builder as HttpClientBuilder};
use clap::Parser;
use tokio::io::AsyncRead;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

use crate::domain::{
    config::{ConfigError, ConfigValidator},
    storage::{StorageError, StorageProvider},
};

/// HTTPS client backed by rustls + ring.
///
/// Avoids the SDK default (`aws-lc-rs` → `aws-lc-sys`), which needs a
/// C/CMake/NASM toolchain and broke cross-platform release builds. Disabling
/// `default-https-client` drops the SDK's auto connector, so this is wired
/// explicitly into the S3 client and the credential/region chains below.
fn https_client() -> SharedHttpClient {
    HttpClientBuilder::new()
        .tls_provider(tls::Provider::Rustls(CryptoMode::Ring))
        .build_https()
}

#[derive(Parser, Debug, Clone)]
pub struct AwsStorageConfig {
    #[arg(
        long,
        env = "AWS_REGION",
        help = "AWS region (e.g., us-west-2). Auto-discovered from environment, AWS config, or EC2/ECS metadata if not provided"
    )]
    pub region: Option<String>,

    #[arg(
        long,
        env = "AWS_ACCESS_KEY_ID",
        help = "AWS access key ID. Optional - uses AWS credential provider chain (environment, config file, IAM roles) if not provided"
    )]
    pub access_key_id: Option<String>,

    #[arg(
        long,
        env = "AWS_SECRET_ACCESS_KEY",
        help = "AWS secret access key. Required if --access-key-id is provided"
    )]
    pub secret_access_key: Option<String>,

    #[arg(
        long,
        env = "AWS_SESSION_TOKEN",
        help = "AWS session token for temporary security credentials. Optional"
    )]
    pub session_token: Option<String>,

    #[arg(
        long,
        env = "S3_BUCKET_NAME",
        help = "S3 bucket name for cache storage"
    )]
    pub bucket_name: String,

    #[arg(
        long,
        env = "S3_ENDPOINT_URL",
        help = "Custom S3 endpoint URL (e.g., http://localhost:9000 for MinIO). Optional - uses AWS S3 if not provided"
    )]
    pub endpoint_url: Option<String>,

    #[arg(
        long,
        env = "S3_TIMEOUT",
        default_value = "30",
        help = "S3 operation timeout in seconds"
    )]
    pub timeout_seconds: u64,
}

impl ProvideRegion for AwsStorageConfig {
    fn region(&self) -> ProvideRegionFuture<'_> {
        let region = self.region.clone();
        ProvideRegionFuture::new(async move {
            // Rebuild the env -> profile -> IMDS chain with our client, since
            // `or_default_provider()` would have no transport without it.
            let provider_config = ProviderConfig::default().with_http_client(https_client());
            RegionProviderChain::first_try(region.map(Region::new))
                .or_else(EnvironmentVariableRegionProvider::new())
                .or_else(
                    ProfileFileRegionProvider::builder()
                        .configure(&provider_config)
                        .build(),
                )
                .or_else(
                    ImdsRegionProvider::builder()
                        .configure(&provider_config)
                        .build(),
                )
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
                // `DefaultCredentialsChain::build()` panics without a configured
                // connector once `default-https-client` is disabled.
                let provider_config = ProviderConfig::default().with_http_client(https_client());
                DefaultCredentialsChain::builder()
                    .configure(provider_config)
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
            return Err(ConfigError::MissingField("S3_BUCKET_NAME"));
        }
        if let Some(endpoint_url) = &self.endpoint_url {
            if !endpoint_url.starts_with("http://") && !endpoint_url.starts_with("https://") {
                return Err(ConfigError::Invalid(
                    "S3 endpoint URL must start with http:// or https://",
                ));
            }
        }
        match (self.access_key_id.as_ref(), self.secret_access_key.as_ref()) {
            (Some(..), None) => return Err(ConfigError::MissingField("AWS_SECRET_ACCESS_KEY")),
            (None, Some(..)) => return Err(ConfigError::MissingField("AWS_ACCESS_KEY_ID")),
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
        // Resolve region once - validation already ensured it exists
        let region = config.region().await.ok_or_else(|| {
            tracing::error!("AWS_REGION must be set");
            StorageError::OperationFailed
        })?;

        let mut s3_config_builder = S3Config::builder()
            .behavior_version_latest()
            .http_client(https_client())
            .region(region)
            .credentials_provider(config.clone())
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(std::time::Duration::from_secs(config.timeout_seconds))
                    .build(),
            );

        // Configure for custom S3-compatible endpoints (MinIO, Hetzner, etc.)
        if let Some(endpoint_url) = &config.endpoint_url {
            s3_config_builder = s3_config_builder
                .endpoint_url(endpoint_url)
                .force_path_style(true); // Required for most S3-compatible services
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
