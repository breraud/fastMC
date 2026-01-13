use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("oauth2 error: {0}")]
    OAuth(String),
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing refresh token from Microsoft")]
    MissingRefreshToken,
    #[error("missing xbox user hash")]
    MissingUserHash,
    #[error("minecraft profile unavailable: {0}")]
    ProfileUnavailable(String),
}
