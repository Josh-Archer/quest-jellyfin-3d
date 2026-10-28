//! Error handling for Jellyfin client operations.

use thiserror::Error;

/// Result alias for Jellyfin client operations.
pub type Result<T> = std::result::Result<T, JellyfinError>;

/// Errors that can occur during Jellyfin API and authentication operations.
#[derive(Debug, Error)]
pub enum JellyfinError {
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Jellyfin API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },

    #[error("Authentication required or token expired")]
    Unauthorized,

    #[error("QuickConnect error: {0}")]
    QuickConnect(String),

    #[error("OIDC authentication error: {0}")]
    Oidc(String),

    #[error("Playback or media format error: {0}")]
    Media(String),

    #[error("Item not found: {0}")]
    NotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}
