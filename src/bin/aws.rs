use clap::Parser;
use nx_cache_server::domain::config::{ConfigValidator, LogFormat, ServerConfig};
use nx_cache_server::infra::aws::{AwsStorageConfig, S3Storage};
use nx_cache_server::logging;
use nx_cache_server::server::run_server;
use std::fmt::Display;

#[derive(Parser)]
#[command(name = "nx-cache-aws")]
#[command(about = "Nx Remote Cache Server - AWS S3 Backend")]
struct AwsCli {
    #[command(flatten)]
    server: ServerConfig,

    #[command(flatten)]
    storage: AwsStorageConfig,
}

/// JSON mode only: in text mode the stderr message would print it twice.
fn log_startup_failure(format: LogFormat, message: &str, error: &dyn Display) {
    if matches!(format, LogFormat::Json) {
        tracing::error!(error = %error, "{message}");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = AwsCli::parse();

    // Before validation, so AWS region/credential warnings are logged.
    let rust_log = std::env::var("RUST_LOG").ok();
    let filter = match logging::resolve_filter(rust_log.as_deref(), cli.server.debug) {
        Ok(filter) => filter,
        Err(e) => {
            eprintln!("Invalid RUST_LOG: {}", e);
            std::process::exit(1);
        }
    };
    let log_format = cli.server.log_format;
    logging::init(log_format, filter.targets);
    if filter.debug_ignored {
        let rust_log = rust_log.unwrap_or_default();
        tracing::warn!(
            rust_log = %rust_log,
            "DEBUG is set but ignored because RUST_LOG is set (RUST_LOG={})",
            rust_log
        );
    }

    // Validate server configuration
    if let Err(e) = cli.server.validate().await {
        log_startup_failure(log_format, "Invalid server configuration", &e);
        eprintln!("{}", e);
        std::process::exit(1);
    }

    // Validate storage configuration
    if let Err(e) = cli.storage.validate().await {
        log_startup_failure(log_format, "Invalid storage configuration", &e);
        eprintln!("{}", e);
        std::process::exit(1);
    }

    // Initialize storage
    let storage = match S3Storage::new(&cli.storage).await {
        Ok(storage) => storage,
        Err(e) => {
            log_startup_failure(log_format, "Failed to initialize S3 storage", &e);
            eprintln!();
            eprintln!("Failed to initialize S3 storage: {}", e);
            eprintln!();
            eprintln!("Please check your AWS credentials and configuration.");
            std::process::exit(1);
        }
    };

    // Run server
    let addr = std::net::SocketAddr::new(cli.server.bind_address, cli.server.port);
    tracing::info!(%addr, "Server starting on {}", addr);
    if let Err(e) = run_server(storage, &cli.server).await {
        log_startup_failure(log_format, "Server error", &e);
        eprintln!();
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
