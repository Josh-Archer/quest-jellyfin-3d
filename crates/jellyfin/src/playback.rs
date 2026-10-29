//! PlaybackInfo request and response models, direct play URL formatting.

use crate::device_profile::DeviceProfile;
use crate::library::MediaStream;
use serde::{Deserialize, Serialize};
use url::Url;

/// Request payload sent to `POST /Items/{id}/PlaybackInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackInfoRequest {
    /// Authenticated Jellyfin User ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Optional resume timestamp in ticks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time_ticks: Option<u64>,
    /// Negotiated client device profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_profile: Option<DeviceProfile>,
    /// Max allowable streaming bitrate in bps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_streaming_bitrate: Option<u64>,
    /// Whether to enable direct play if supported.
    #[serde(default = "default_true")]
    pub enable_direct_play: bool,
    /// Whether to enable direct stream if supported.
    #[serde(default = "default_true")]
    pub enable_direct_stream: bool,
    /// Whether to enable transcoding if supported.
    #[serde(default = "default_true")]
    pub enable_transcoding: bool,
}

fn default_true() -> bool {
    true
}

impl PlaybackInfoRequest {
    /// Creates a default PlaybackInfo request for a user with the Quest device profile.
    pub fn new(user_id: Option<String>, profile: DeviceProfile) -> Self {
        Self {
            user_id,
            start_time_ticks: None,
            device_profile: Some(profile),
            max_streaming_bitrate: Some(120_000_000),
            enable_direct_play: true,
            enable_direct_stream: true,
            enable_transcoding: true,
        }
    }
}

/// A media source returned by Jellyfin PlaybackInfo.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSource {
    /// Media source ID.
    pub id: String,
    /// Container format (e.g. "mkv", "mp4").
    pub container: Option<String>,
    /// Server filesystem path.
    pub path: Option<String>,
    /// Whether the Jellyfin server confirmed this source can be direct played.
    #[serde(default)]
    pub supports_direct_play: bool,
    /// Whether direct streaming (remuxing) is supported.
    #[serde(default)]
    pub supports_direct_stream: bool,
    /// Whether transcoding is supported.
    #[serde(default)]
    pub supports_transcoding: bool,
    /// Direct stream URL provided by server if direct play is not possible.
    pub direct_stream_url: Option<String>,
    /// Total bitrate in bps.
    pub bitrate: Option<u64>,
    /// List of media streams.
    #[serde(default)]
    pub media_streams: Vec<MediaStream>,
}

/// Response returned from `POST /Items/{id}/PlaybackInfo`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackInfoResponse {
    /// Candidate media sources for playback.
    #[serde(default)]
    pub media_sources: Vec<MediaSource>,
    /// Unique session identifier assigned by Jellyfin for this playback attempt.
    pub play_session_id: Option<String>,
    /// Error code if playback is not allowed.
    pub error_code: Option<String>,
}

/// Direct play stream URL: `{base_url}/Videos/{id}/stream?static=true[&api_key=..]`.
///
/// `base_url` is the client's normalized base URL (no trailing slash, subpath kept).
///
/// # Errors
///
/// Returns [`crate::Error::InvalidUrl`] when the result is not a valid URL.
pub fn build_direct_play_url(
    base_url: &str,
    item_id: &str,
    token: Option<&str>,
) -> crate::JellyfinResult<Url> {
    let base = base_url.trim_end_matches('/');
    let id = crate::identity::encode_component(item_id);
    let mut url = Url::parse(&format!("{base}/Videos/{id}/stream"))
        .map_err(|err| crate::Error::InvalidUrl(err.to_string()))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("static", "true");
        if let Some(tok) = token {
            query.append_pair("api_key", tok);
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_direct_play_url_without_token() {
        let url =
            build_direct_play_url("https://jellyfin.archerfamily.io", "item-12345", None).unwrap();
        assert_eq!(
            url.as_str(),
            "https://jellyfin.archerfamily.io/Videos/item-12345/stream?static=true"
        );
    }

    #[test]
    fn test_build_direct_play_url_with_token() {
        let url = build_direct_play_url(
            "https://jellyfin.archerfamily.io",
            "item-12345",
            Some("tok-abc"),
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://jellyfin.archerfamily.io/Videos/item-12345/stream?static=true&api_key=tok-abc"
        );
    }

    #[test]
    fn test_deserialize_playback_info_response() {
        let json = r#"{
            "PlaySessionId": "sess-xyz",
            "MediaSources": [
                {
                    "Id": "source-1",
                    "Container": "mkv",
                    "SupportsDirectPlay": true,
                    "SupportsDirectStream": true,
                    "SupportsTranscoding": false,
                    "Bitrate": 45000000
                }
            ]
        }"#;
        let res: PlaybackInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(res.play_session_id.as_deref(), Some("sess-xyz"));
        assert_eq!(res.media_sources.len(), 1);
        assert!(res.media_sources[0].supports_direct_play);
    }
}
