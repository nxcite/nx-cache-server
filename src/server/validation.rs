use crate::server::error::ServerError;

pub fn validate_hash(hash: &str) -> Result<(), ServerError> {
    if hash.is_empty() {
        return Err(ServerError::BadRequest);
    }
    
    if !hash.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(ServerError::BadRequest);
    }
    
    if hash.len() > 128 {
        return Err(ServerError::BadRequest);
    }
    
    Ok(())
}