//! Error handling for quest-jellyfin-player.

use thiserror::Error;

/// Result type alias for player operations.
pub type Result<T> = std::result::Result<T, PlayerError>;

/// Errors encountered during playback or rendering.
#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("Jellyfin error: {0}")]
    Jellyfin(#[from] jellyfin_client::Error),

    #[error("Video decode error: {0}")]
    Decode(#[from] xr_video::MediaError),

    #[error("OpenXR error: {0}")]
    OpenXr(#[from] xr_core::XrError),

    #[error("Stereo layout error: {0}")]
    StereoLayout(String),

    #[error("Playback engine error: {0}")]
    Playback(String),
}
