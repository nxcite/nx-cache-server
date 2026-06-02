use clap::Parser;
use nx_cache_server::domain::config::{ConfigValidator, LogLevel, ServerConfig};
use nx_cache_server::infra::aws::{AwsStorageConfig, S3Storage};
use nx_cache_server::server::run_server;
use tracing::Level;

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
    let cli = AwsCli::parse();

    // Resolve log verbosity: --log-level wins, then --debug, otherwise info.
    let log_level = match cli.server.log_level {
        Some(level) => level,
        None if cli.server.debug => LogLevel::Debug,
        None => LogLevel::Info,
    };
    let max_level = match log_level {
        LogLevel::Trace => Level::TRACE,
        LogLevel::Debug => Level::DEBUG,
        LogLevel::Info => Level::INFO,
        LogLevel::Warn => Level::WARN,
        LogLevel::Error => Level::ERROR,
    };

    // Initialize logging
    tracing_subscriber::fmt().with_max_level(max_level).init();

    // Validate server configuration
    if let Err(e) = cli.server.validate().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    // Validate storage configuration
    if let Err(e) = cli.storage.validate().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    // Initialize storage
    let storage = match S3Storage::new(&cli.storage).await {
        Ok(storage) => storage,
        Err(e) => {
            eprintln!();
            eprintln!("Failed to initialize S3 storage: {}", e);
            eprintln!();
            eprintln!("Please check your AWS credentials and configuration.");
            std::process::exit(1);
        }
    };

    // Run server
    tracing::info!("Server starting on port {}", cli.server.port);
    if let Err(e) = run_server(storage, &cli.server).await {
        eprintln!();
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
