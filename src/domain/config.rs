use clap::Parser;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("Invalid configuration: {0}")]
    Invalid(&'static str),
}

pub trait ConfigValidator {
    fn validate(&self) -> Result<(), ConfigError>;
}

#[derive(Parser, Debug, Clone)]
pub struct ServerConfig {
    /// HTTP server port
    #[arg(long, env = "PORT", default_value = "3000")]
    pub port: u16,
    
    /// Bearer token for client authentication
    #[arg(long, env = "SERVICE_ACCESS_TOKEN")]
    pub service_access_token: String,
    
    /// Enable debug logging
    #[arg(long, env = "DEBUG")]
    pub debug: bool,
}

impl ConfigValidator for ServerConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.service_access_token.is_empty() {
            return Err(ConfigError::MissingField("service access token"));
        }
        
        if self.port == 0 {
            return Err(ConfigError::Invalid("port must be greater than 0"));
        }
        
        Ok(())
    }
}