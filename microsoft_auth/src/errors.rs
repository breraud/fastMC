use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("oauth2 error: {0}")]
    OAuth(String),
}
