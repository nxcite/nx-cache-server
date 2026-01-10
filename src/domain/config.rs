use clap::Parser;
use std::collections::HashMap;
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
                            "Service access token(s) are required for client authentication."
                        )?;
                        writeln!(f)?;
                        writeln!(f, "Provide access token(s) via:")?;
                        writeln!(f, "  1. --service-access-token command line argument")?;
                        writeln!(f, "  2. SERVICE_ACCESS_TOKEN environment variable")?;
                        writeln!(f)?;
                        writeln!(f, "Supported formats:")?;
                        writeln!(f, "  - Single plain: mytoken")?;
                        writeln!(f, "  - Single named: production=mytoken")?;
                        writeln!(f, "  - Multiple plain: token1,token2,token3")?;
                        writeln!(f, "  - Multiple named: frontend=token1,backend=token2")?;
                        writeln!(f, "  - Mixed: frontend=token1,token2,ci=token3")?;
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

// Token registry for named tokens with reverse lookup
#[derive(Debug, Clone)]
pub struct TokenRegistry {
    tokens: HashMap<String, String>,  // name -> token
    reverse: HashMap<String, String>, // token -> name (for lookup)
}

impl TokenRegistry {
    pub fn from_strings(raw_tokens: &[String]) -> Result<Self, ConfigError> {
        let mut tokens = HashMap::new();
        let mut reverse = HashMap::new();
        let mut unnamed_counter = 1;

        for raw in raw_tokens {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }

            let (name, token) = if let Some((name_part, token_part)) = raw.split_once('=') {
                // New format: name=token
                let name = name_part.trim().to_string();
                let token = token_part.trim().to_string();

                if name.is_empty() || token.is_empty() {
                    return Err(ConfigError::Invalid("Token name and value cannot be empty"));
                }

                (name, token)
            } else {
                // Old format: plain token (backwards compatibility)
                let token = raw.to_string();
                let name = if unnamed_counter == 1 {
                    "default".to_string()
                } else {
                    format!("token-{}", unnamed_counter)
                };
                unnamed_counter += 1;
                (name, token)
            };

            tokens.insert(name.clone(), token.clone());
            reverse.insert(token, name);
        }

        if tokens.is_empty() {
            return Err(ConfigError::MissingField("SERVICE_ACCESS_TOKEN"));
        }

        Ok(Self { tokens, reverse })
    }

    pub fn find_token_name(&self, token: &str) -> Option<&str> {
        self.reverse.get(token).map(|s| s.as_str())
    }

    pub fn tokens(&self) -> impl Iterator<Item = &String> {
        self.tokens.values()
    }

    pub fn token_names(&self) -> impl Iterator<Item = &String> {
        self.tokens.keys()
    }
}

#[derive(Parser, Debug, Clone)]
pub struct ServerConfig {
    #[arg(long, env = "PORT", default_value = "3000", help = "HTTP server port")]
    pub port: u16,

    #[arg(
        long = "service-access-token",
        env = "SERVICE_ACCESS_TOKEN",
        value_delimiter = ',',
        help = "Bearer token(s) for client authentication. Supports: single token, multiple tokens (comma-separated), named tokens (name=token), or mixed formats"
    )]
    pub service_access_token: Vec<String>,

    #[arg(long, env = "DEBUG", help = "Enable debug logging")]
    pub debug: bool,
}

impl ConfigValidator for ServerConfig {
    async fn validate(&self) -> Result<(), ConfigError> {
        // Validate token format and ensure at least one token exists
        TokenRegistry::from_strings(&self.service_access_token)?;

        if self.port == 0 {
            return Err(ConfigError::Invalid("port must be greater than 0"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_tokens() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec![
                "frontend=abc123".to_string(),
                "backend=def456".to_string(),
                "ci=xyz789".to_string(),
            ],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();

        assert_eq!(registry.find_token_name("abc123"), Some("frontend"));
        assert_eq!(registry.find_token_name("def456"), Some("backend"));
        assert_eq!(registry.find_token_name("xyz789"), Some("ci"));
        assert_eq!(registry.find_token_name("invalid"), None);
    }

    #[test]
    fn test_plain_tokens_backwards_compatible() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec![
                "abc123".to_string(),
                "def456".to_string(),
                "xyz789".to_string(),
            ],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();

        // First token gets "default", rest get "token-N"
        assert_eq!(registry.find_token_name("abc123"), Some("default"));
        assert_eq!(registry.find_token_name("def456"), Some("token-2"));
        assert_eq!(registry.find_token_name("xyz789"), Some("token-3"));
    }

    #[test]
    fn test_mixed_tokens() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec![
                "frontend=abc123".to_string(),
                "def456".to_string(),
                "ci=xyz789".to_string(),
            ],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();

        assert_eq!(registry.find_token_name("abc123"), Some("frontend"));
        assert_eq!(registry.find_token_name("def456"), Some("default"));
        assert_eq!(registry.find_token_name("xyz789"), Some("ci"));
    }

    #[test]
    fn test_single_plain_token() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec!["mytoken123".to_string()],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();

        assert_eq!(registry.find_token_name("mytoken123"), Some("default"));
    }

    #[test]
    fn test_empty_tokens_error() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec![],
            debug: false,
        };

        let result = TokenRegistry::from_strings(&config.service_access_token);

        assert!(result.is_err());
        match result {
            Err(ConfigError::MissingField("SERVICE_ACCESS_TOKEN")) => {}
            _ => panic!("Expected MissingField error"),
        }
    }

    #[test]
    fn test_empty_name_error() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec!["=token123".to_string()],
            debug: false,
        };

        let result = TokenRegistry::from_strings(&config.service_access_token);

        assert!(result.is_err());
        match result {
            Err(ConfigError::Invalid(_)) => {}
            _ => panic!("Expected Invalid error"),
        }
    }

    #[test]
    fn test_empty_token_value_error() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec!["frontend=".to_string()],
            debug: false,
        };

        let result = TokenRegistry::from_strings(&config.service_access_token);

        assert!(result.is_err());
        match result {
            Err(ConfigError::Invalid(_)) => {}
            _ => panic!("Expected Invalid error"),
        }
    }

    #[test]
    fn test_whitespace_handling() {
        let tokens = vec![" frontend = abc123 ".to_string(), "  def456  ".to_string()];

        let registry = TokenRegistry::from_strings(&tokens).unwrap();

        assert_eq!(registry.find_token_name("abc123"), Some("frontend"));
        assert_eq!(registry.find_token_name("def456"), Some("default"));
    }

    #[test]
    fn test_empty_string_tokens_are_skipped() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec![
                "frontend=abc123".to_string(),
                "".to_string(),
                "  ".to_string(),
                "backend=def456".to_string(),
            ],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();

        assert_eq!(registry.find_token_name("abc123"), Some("frontend"));
        assert_eq!(registry.find_token_name("def456"), Some("backend"));
    }

    #[test]
    fn test_single_token_plain() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec!["oldtoken123".to_string()],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();
        assert_eq!(registry.find_token_name("oldtoken123"), Some("default"));
    }

    #[test]
    fn test_single_token_named() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec!["frontend=token123".to_string()],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();
        assert_eq!(registry.find_token_name("token123"), Some("frontend"));
    }

    #[test]
    fn test_multiple_tokens_all_formats() {
        let config = ServerConfig {
            port: 3000,
            service_access_token: vec![
                "frontend=newtoken1".to_string(),
                "backend=newtoken2".to_string(),
                "oldtoken".to_string(),
            ],
            debug: false,
        };

        let registry = TokenRegistry::from_strings(&config.service_access_token).unwrap();
        assert_eq!(registry.find_token_name("newtoken1"), Some("frontend"));
        assert_eq!(registry.find_token_name("newtoken2"), Some("backend"));
        assert_eq!(registry.find_token_name("oldtoken"), Some("default"));
    }
}
