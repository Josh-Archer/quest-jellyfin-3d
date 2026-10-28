//! Jellyfin library item models, stream metadata, and browse queries.

use serde::{Deserialize, Serialize};

/// 3D video layout formats recognized directly by Jellyfin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JellyfinVideo3DFormat {
    FullSideBySide,
    HalfSideBySide,
    FullTopAndBottom,
    HalfTopAndBottom,
    Mvc,
}

/// Media stream type in Jellyfin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamType {
    Video,
    Audio,
    Subtitle,
    EmbeddedImage,
    Data,
    #[serde(other)]
    Unknown,
}

/// Detailed metadata about an individual stream within a media item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct MediaStream {
    /// Stream codec name (e.g. "hevc", "av1", "h264", "aac", "opus").
    pub codec: Option<String>,
    /// Stream media type.
    #[serde(rename = "Type")]
    pub stream_type: Option<StreamType>,
    /// Stream index in container.
    pub index: Option<i32>,
    /// Pixel width (video only).
    pub width: Option<u32>,
    /// Pixel height (video only).
    pub height: Option<u32>,
    /// Display aspect ratio string (e.g. "16:9", "2.39:1").
    pub aspect_ratio: Option<String>,
    /// Nominal or average bitrate in bps.
    pub bit_rate: Option<u64>,
    /// Average framerate in fps.
    pub average_frame_rate: Option<f32>,
    /// Real framerate in fps.
    pub real_frame_rate: Option<f32>,
    /// Stereo packing mode tag (e.g. "left_right", "top_bottom", or MKV stereo flag).
    pub stereo_mode: Option<String>,
    /// Whether this is the default track for this stream type.
    pub is_default: Option<bool>,
    /// User-facing display title.
    pub display_title: Option<String>,
    /// Language code (e.g. "eng", "jpn").
    pub language: Option<String>,
}

/// User-specific playback data and progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct UserData {
    /// Playback resume position in Jellyfin ticks (1 tick = 100 ns).
    pub playback_position_ticks: Option<u64>,
    /// Number of times played.
    pub play_count: Option<u32>,
    /// Whether marked as favorite.
    pub is_favorite: Option<bool>,
    /// Whether marked as played/watched.
    pub played: Option<bool>,
}

/// A media item (movie, episode, video) returned by Jellyfin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaItem {
    /// Jellyfin item GUID.
    pub id: String,
    /// Item title.
    pub name: String,
    /// Original item title.
    pub original_title: Option<String>,
    /// Server filesystem path (e.g. "/Volume1/media/3D/Avatar (2009).3D.HSBS.mkv").
    pub path: Option<String>,
    /// Jellyfin native 3D format tag if present.
    #[serde(rename = "Video3DFormat", alias = "Video3dFormat")]
    pub video_3d_format: Option<JellyfinVideo3DFormat>,
    /// List of media streams (video, audio, subtitle).
    #[serde(default)]
    pub media_streams: Vec<MediaStream>,
    /// Container format (e.g. "mkv", "mp4").
    pub container: Option<String>,
    /// Runtime in ticks (1 sec = 10,000,000 ticks).
    pub run_time_ticks: Option<u64>,
    /// Plot summary.
    pub overview: Option<String>,
    /// Item type (e.g. "Movie", "Episode", "Video", "Folder").
    #[serde(rename = "Type")]
    pub item_type: Option<String>,
    /// User playback status.
    pub user_data: Option<UserData>,
}

/// Query parameters for browsing library items (`GET /Items`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemsQuery {
    /// Parent folder / library ID.
    pub parent_id: Option<String>,
    /// Include only specific item types (e.g. `["Movie", "Episode"]`).
    pub include_item_types: Vec<String>,
    /// Fields requested from server (must include `Video3DFormat,MediaStreams,Path`).
    pub fields: Vec<String>,
    /// Sort attribute (e.g. "SortName", "DateCreated").
    pub sort_by: Option<String>,
    /// Sort order ("Ascending", "Descending").
    pub sort_order: Option<String>,
    /// Zero-based pagination start index.
    pub start_index: Option<u32>,
    /// Page size limit.
    pub limit: Option<u32>,
    /// Recursive search under parent.
    pub recursive: bool,
}

impl Default for ItemsQuery {
    fn default() -> Self {
        Self {
            parent_id: None,
            include_item_types: vec![
                "Movie".to_string(),
                "Episode".to_string(),
                "Video".to_string(),
            ],
            fields: vec![
                "Video3DFormat".to_string(),
                "MediaStreams".to_string(),
                "Path".to_string(),
                "Overview".to_string(),
                "UserData".to_string(),
            ],
            sort_by: Some("SortName".to_string()),
            sort_order: Some("Ascending".to_string()),
            start_index: None,
            limit: None,
            recursive: true,
        }
    }
}

/// Response returned by `GET /Items`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ItemsResponse {
    /// Retrieved items.
    #[serde(default)]
    pub items: Vec<MediaItem>,
    /// Total count of matching items in the collection.
    pub total_record_count: u32,
    /// Starting index of this page.
    pub start_index: Option<u32>,
}
