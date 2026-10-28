//! Pure Rust, host-testable Jellyfin client library for `quest-jellyfin-3d`.
//!
//! Provides:
//! - QuickConnect pairing flow (`POST /QuickConnect/Initiate`, `GET /QuickConnect/Connect`, and `/Users/AuthenticateWithQuickConnect`)
//! - Keycloak OpenID Connect (OIDC) stub and interface
//! - Library browse with 3D fields (`GET /Items?Fields=Video3DFormat,MediaStreams,Path`)
//! - PlaybackInfo negotiation with Meta Quest 3+ device profiles (HEVC/AV1 direct play, ~8K30 HEVC cap)
//! - Direct play URL generation (`/Videos/{id}/stream?static=true`)
//! - Session reporting (`/Sessions/Playing`, `/Progress`, `/Stopped`)
//! - 3D format detection prioritizing `Video3DFormat`, filename tags, and container metadata

pub mod auth;
pub mod client;
pub mod device_profile;
pub mod error;
pub mod format_detection;
pub mod models;
pub mod oidc;
pub mod playback;
pub mod quick_connect;
pub mod sessions;

pub use auth::{ClientInfo, Session};
pub use client::JellyfinClient;
pub use device_profile::{
    evaluate_direct_play, quest_device_profile, CodecProfile, DeviceProfile, DirectPlayDecision,
    DirectPlayProfile, ProfileCondition,
};
pub use error::{JellyfinError, Result};
pub use format_detection::{
    detect_from_container, detect_from_filename, detect_stereo_format, DetectedStereoFormat,
    DetectionSource, EyeOrder, ProjectionType, StereoFormat,
};
pub use models::{
    ItemsQuery, ItemsResponse, JellyfinVideo3DFormat, MediaItem, MediaStream, StreamType, UserData,
};
pub use oidc::{
    KeycloakOidcClient, MockOidcProvider, OidcConfig, OidcProvider, OidcTokenResponse, OidcUserInfo,
};
pub use playback::{build_direct_play_url, MediaSource, PlaybackInfoRequest, PlaybackInfoResponse};
pub use quick_connect::{
    QuickConnectAuthRequest, QuickConnectAuthResponse, QuickConnectInitiateResponse,
    QuickConnectSessionInfo, QuickConnectStateResponse, QuickConnectUser,
};
pub use sessions::{
    PlayMethod, PlaybackProgressRequest, PlaybackStartRequest, PlaybackStopRequest,
};
