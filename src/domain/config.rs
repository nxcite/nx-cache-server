use clap::Parser;
use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    MissingField(&'static str),
    Invalid(&'static str),
}

impl std::error::Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingField(field) => {
                writeln!(f)?;
                writeln!(f, "Configuration Error: Missing required field")?;
                writeln!(f)?;

                match *field {
                    "AWS_REGION" => {
                        writeln!(f, "Could not determine AWS region.")?;
                        writeln!(
                            f,
                            "Pls provide it via --region or AWS_REGION environment variable."
                        )?;
                        writeln!(f)?;
                        writeln!(
                            f,
                            "Note: If you see IMDS timeout warnings above, it means the program"
                        )?;
                        writeln!(
                            f,
                            "      attempted to auto-discover the region from EC2 metadata but"
                        )?;
                        writeln!(f, "      you are not running on AWS infrastructure.")?;
                    }
                    "AWS_SECRET_ACCESS_KEY" => {
                        writeln!(f, "AWS credentials must be provided as a complete pair.")?;
                        writeln!(f)?;
                        writeln!(
                            f,
                            "You provided AWS_ACCESS_KEY_ID but not AWS_SECRET_ACCESS_KEY."
                        )?;
                        writeln!(f)?;
                        writeln!(
                            f,
                            "Alternatively, omit both to use AWS credential provider chain"
                        )?;
                        writeln!(f, "(IAM roles, instance profiles, etc.).")?;
                    }
                    "AWS_ACCESS_KEY_ID" => {
                        writeln!(f, "AWS credentials must be provided as a complete pair.")?;
                        writeln!(f)?;
                        writeln!(
                            f,
                            "You provided AWS_SECRET_ACCESS_KEY but not AWS_ACCESS_KEY_ID."
                        )?;
                        writeln!(f)?;
                        writeln!(
                            f,
                            "Alternatively, omit both to use AWS credential provider chain"
                        )?;
                        writeln!(f, "(IAM roles, instance profiles, etc.).")?;
                    }
                    "S3_BUCKET_NAME" => {
                        writeln!(f, "S3 bucket name is required.")?;
                        writeln!(f)?;
                        writeln!(f, "Provide the S3 bucket name via:")?;
                        writeln!(f, "  1. --bucket-name command line argument")?;
                        writeln!(f, "  2. S3_BUCKET_NAME environment variable")?;
                    }
                    "SERVICE_ACCESS_TOKEN" => {
                        writeln!(
                            f,
                            "Service access token is required for client authentication."
                        )?;
                        writeln!(f)?;
                        writeln!(f, "Provide the service access token via:")?;
                        writeln!(f, "  1. --service-access-token command line argument")?;
                        writeln!(f, "  2. SERVICE_ACCESS_TOKEN environment variable")?;
                        writeln!(f)?;
                        writeln!(
                            f,
                            "This token must match the token configured in your Nx clients."
                        )?;
                    }
                    _ => {
                        writeln!(f, "Field: {}", field)?;
                        writeln!(f)?;
                        writeln!(f, "Please provide this required configuration parameter.")?;
                    }
                }
            }
            ConfigError::Invalid(msg) => {
                writeln!(f)?;
                writeln!(f, "Configuration Error: Invalid value")?;
                writeln!(f)?;
                writeln!(f, "{}", msg)?;
                writeln!(f)?;
            }
        }

        writeln!(f, "Run with --help for more information.")
    }
}

pub trait ConfigValidator {
    fn validate(&self) -> impl std::future::Future<Output = Result<(), ConfigError>>;
}

#[derive(Parser, Debug, Clone)]
pub struct ServerConfig {
    #[arg(long, env = "PORT", default_value = "3000", help = "HTTP server port")]
    pub port: u16,

    #[arg(
        long,
        env = "SERVICE_ACCESS_TOKEN",
        help = "Bearer token for client authentication"
    )]
    pub service_access_token: String,

    #[arg(long, env = "DEBUG", help = "Enable debug logging")]
    pub debug: bool,
}

impl ConfigValidator for ServerConfig {
    async fn validate(&self) -> Result<(), ConfigError> {
        if self.service_access_token.is_empty() {
            return Err(ConfigError::MissingField("SERVICE_ACCESS_TOKEN"));
        }

        if self.port == 0 {
            return Err(ConfigError::Invalid("port must be greater than 0"));
        }

        Ok(())
    }
}
