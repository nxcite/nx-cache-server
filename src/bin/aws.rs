use clap::Parser;
use nx_cache_server::domain::config::{ConfigValidator, ServerConfig};
use nx_cache_server::infra::aws::{AwsStorageConfig, S3Storage};
use nx_cache_server::server::run_server;

#[derive(Parser)]
#[command(name = "nx-cache-aws")]
#[command(about = "Nx Remote Cache Server - AWS S3 Backend")]
struct AwsCli {
    #[command(flatten)]
    server: ServerConfig,

    #[command(flatten)]
    storage: AwsStorageConfig,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = AwsCli::parse();

    // Two validation calls
    cli.server.validate()?;
    cli.storage.validate()?;

    // Clean interfaces - each component gets exactly what it needs
    let storage = S3Storage::new(&cli.storage).await?;
    run_server(storage, &cli.server).await?;

    Ok(())
}
