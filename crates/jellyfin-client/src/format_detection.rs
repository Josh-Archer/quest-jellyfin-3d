//! Stereo 3D format detection prioritizing Jellyfin metadata, filename tags, and container metadata.

use crate::models::{JellyfinVideo3DFormat, MediaItem, StreamType};
use serde::{Deserialize, Serialize};

/// Stereo packing format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StereoFormat {
    /// Standard 2D video (monoscopic).
    Mono,
    /// Half Side-by-Side (horizontally compressed, 2x aspect stretch).
    HalfSideBySide,
    /// Full Side-by-Side (discrete double-width frame).
    FullSideBySide,
    /// Half Over-Under / Top-and-Bottom (vertically compressed, 2x aspect stretch).
    HalfOverUnder,
    /// Full Over-Under / Top-and-Bottom (discrete double-height frame).
    FullOverUnder,
    /// Multi-view Video Coding (Blu-ray 3D).
    Mvc,
}

impl StereoFormat {
    /// Returns true if this format is any stereoscopic 3D format.
    pub fn is_3d(&self) -> bool {
        !matches!(self, Self::Mono)
    }

    /// Returns true if this is a side-by-side format.
    pub fn is_sbs(&self) -> bool {
        matches!(self, Self::HalfSideBySide | Self::FullSideBySide)
    }

    /// Returns true if this is an over-under / top-and-bottom format.
    pub fn is_over_under(&self) -> bool {
        matches!(self, Self::HalfOverUnder | Self::FullOverUnder)
    }
}

/// Eye ordering for packed stereoscopic frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EyeOrder {
    /// Left eye in first region (left half or top half).
    #[default]
    LeftRight,
    /// Right eye in first region (right half or bottom half).
    RightLeft,
    /// Top eye is left eye (standard over-under).
    TopBottom,
    /// Bottom eye is left eye (inverted over-under).
    BottomTop,
}

/// Immersive projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProjectionType {
    /// Rectilinear flat or curved virtual screen in 3D space.
    #[default]
    FlatVirtualScreen,
    /// 180° hemispherical equirectangular projection.
    Vr180,
    /// 360° spherical equirectangular projection.
    Vr360,
    /// Fisheye lens projection (e.g. Canon dual fisheye RF 5.2mm).
    Fisheye,
}

/// Source that determined the stereo format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectionSource {
    /// Jellyfin server `Video3DFormat` item attribute.
    Jellyfin3DFormat,
    /// Filename tags (3D, HSBS, FSBS, OU, TAB, 180, 360, fisheye, _LR, _TB).
    FilenameTag,
    /// Container or stream metadata (MKV StereoMode or stream tags).
    ContainerMetadata,
    /// Fallback default (standard 2D Mono).
    FallbackDefault,
}

/// Comprehensive stereo 3D configuration detected for a video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedStereoFormat {
    /// Stereo packing layout.
    pub format: StereoFormat,
    /// Eye order packing.
    pub eye_order: EyeOrder,
    /// Projection geometry.
    pub projection: ProjectionType,
    /// Which detection layer made the determination.
    pub detection_source: DetectionSource,
}

impl Default for DetectedStereoFormat {
    fn default() -> Self {
        Self {
            format: StereoFormat::Mono,
            eye_order: EyeOrder::LeftRight,
            projection: ProjectionType::FlatVirtualScreen,
            detection_source: DetectionSource::FallbackDefault,
        }
    }
}

/// Detects the stereo format according to the milestone priority order:
/// 1. Jellyfin `Video3DFormat`
/// 2. Filename tags (`3D`, `HSBS`, `FSBS`, `OU`, `TAB`, `180`, `360`, `fisheye`, `_LR`/`_TB`, etc.)
/// 3. Container metadata (MKV `StereoMode`, stream `stereo_mode` tag)
/// 4. Fallback default (Mono)
pub fn detect_stereo_format(item: &MediaItem) -> DetectedStereoFormat {
    // 1. Jellyfin Video3DFormat
    if let Some(format_3d) = item.video_3d_format {
        let (format, eye_order) = match format_3d {
            JellyfinVideo3DFormat::HalfSideBySide => {
                (StereoFormat::HalfSideBySide, EyeOrder::LeftRight)
            }
            JellyfinVideo3DFormat::FullSideBySide => {
                (StereoFormat::FullSideBySide, EyeOrder::LeftRight)
            }
            JellyfinVideo3DFormat::HalfTopAndBottom => {
                (StereoFormat::HalfOverUnder, EyeOrder::TopBottom)
            }
            JellyfinVideo3DFormat::FullTopAndBottom => {
                (StereoFormat::FullOverUnder, EyeOrder::TopBottom)
            }
            JellyfinVideo3DFormat::Mvc => (StereoFormat::Mvc, EyeOrder::LeftRight),
        };

        // Even when Video3DFormat is set, filename tags may refine projection (VR180/360) or eye swap (_RL/_BT)
        let (projection, refined_eye_order) =
            if let Some(path_or_name) = item.path.as_deref().or(Some(&item.name)) {
                let p = detect_projection_from_str(path_or_name);
                let eo = detect_eye_order_from_str(path_or_name).unwrap_or(eye_order);
                (p, eo)
            } else {
                (ProjectionType::FlatVirtualScreen, eye_order)
            };

        return DetectedStereoFormat {
            format,
            eye_order: refined_eye_order,
            projection,
            detection_source: DetectionSource::Jellyfin3DFormat,
        };
    }

    // 2. Filename tags
    let filename_target = item.path.as_deref().unwrap_or(&item.name);
    if let Some(detected) = detect_from_filename(filename_target) {
        return detected;
    }

    // 3. Container metadata
    if let Some(detected) = detect_from_container(item) {
        return detected;
    }

    // 4. Default fallback
    DetectedStereoFormat::default()
}

/// Helper to parse filename tags.
pub fn detect_from_filename(filename: &str) -> Option<DetectedStereoFormat> {
    let lower = filename.to_lowercase();
    let tokens = tokenize_filename(&lower);

    let has_token = |t: &str| tokens.iter().any(|tok| tok == t);
    let has_prefix = |prefix: &str| tokens.iter().any(|tok| tok.starts_with(prefix));

    // Projection
    let projection = detect_projection_from_tokens(&tokens);

    // Eye order
    let eye_order = detect_eye_order_from_tokens(&tokens);

    // Compound pattern checks
    let is_full_sbs = has_token("fsbs")
        || has_token("fullsbs")
        || (has_token("full") && has_token("sbs"))
        || lower.contains("full-sbs")
        || lower.contains("full_sbs")
        || lower.contains("full.sbs");

    let is_half_sbs = has_token("hsbs")
        || has_token("halfsbs")
        || (has_token("half") && has_token("sbs"))
        || lower.contains("half-sbs")
        || lower.contains("half_sbs")
        || lower.contains("half.sbs")
        || lower.contains("h-sbs")
        || lower.contains("h_sbs");

    let is_full_ou = has_token("fou")
        || has_token("fullou")
        || has_token("ftab")
        || has_token("fulltab")
        || (has_token("full") && (has_token("ou") || has_token("tab")))
        || lower.contains("full-ou")
        || lower.contains("full-tab")
        || lower.contains("full_ou")
        || lower.contains("full_tab");

    let is_half_ou = has_token("hou")
        || has_token("halfou")
        || has_token("htab")
        || has_token("halftab")
        || (has_token("half") && (has_token("ou") || has_token("tab")))
        || lower.contains("half-ou")
        || lower.contains("half-tab")
        || lower.contains("h-tab")
        || lower.contains("h-ou")
        || has_token("tab")
        || has_token("ou")
        || lower.contains("over-under")
        || lower.contains("top-bottom");

    // Stereo layout
    let format = if is_full_sbs {
        Some(StereoFormat::FullSideBySide)
    } else if is_half_sbs {
        Some(StereoFormat::HalfSideBySide)
    } else if is_full_ou {
        Some(StereoFormat::FullOverUnder)
    } else if is_half_ou {
        Some(StereoFormat::HalfOverUnder)
    } else if has_token("sbs") || lower.contains("side-by-side") || has_token("sidebyside") {
        Some(StereoFormat::HalfSideBySide)
    } else if has_token("3d") || has_prefix("3d_") || has_prefix("3d-") {
        // Generic 3D tag: default to HalfSideBySide
        Some(StereoFormat::HalfSideBySide)
    } else if projection != ProjectionType::FlatVirtualScreen {
        // VR180 / VR360 video defaults to SBS stereo
        Some(StereoFormat::HalfSideBySide)
    } else {
        None
    };

    format.map(|fmt| DetectedStereoFormat {
        format: fmt,
        eye_order: eye_order.unwrap_or_else(|| {
            if fmt.is_over_under() {
                EyeOrder::TopBottom
            } else {
                EyeOrder::LeftRight
            }
        }),
        projection,
        detection_source: DetectionSource::FilenameTag,
    })
}

/// Helper to parse container / stream metadata.
pub fn detect_from_container(item: &MediaItem) -> Option<DetectedStereoFormat> {
    for stream in &item.media_streams {
        if stream.stream_type == Some(StreamType::Video) {
            if let Some(mode) = &stream.stereo_mode {
                let lower = mode.trim().to_lowercase();
                match lower.as_str() {
                    "1" | "side_by_side" | "left_right" | "sbs" => {
                        return Some(DetectedStereoFormat {
                            format: StereoFormat::HalfSideBySide,
                            eye_order: EyeOrder::LeftRight,
                            projection: ProjectionType::FlatVirtualScreen,
                            detection_source: DetectionSource::ContainerMetadata,
                        });
                    }
                    "11" | "right_left" => {
                        return Some(DetectedStereoFormat {
                            format: StereoFormat::HalfSideBySide,
                            eye_order: EyeOrder::RightLeft,
                            projection: ProjectionType::FlatVirtualScreen,
                            detection_source: DetectionSource::ContainerMetadata,
                        });
                    }
                    "2" | "bottom_top" => {
                        return Some(DetectedStereoFormat {
                            format: StereoFormat::HalfOverUnder,
                            eye_order: EyeOrder::BottomTop,
                            projection: ProjectionType::FlatVirtualScreen,
                            detection_source: DetectionSource::ContainerMetadata,
                        });
                    }
                    "3" | "top_bottom" | "top_and_bottom" => {
                        return Some(DetectedStereoFormat {
                            format: StereoFormat::HalfOverUnder,
                            eye_order: EyeOrder::TopBottom,
                            projection: ProjectionType::FlatVirtualScreen,
                            detection_source: DetectionSource::ContainerMetadata,
                        });
                    }
                    "0" | "mono" => {
                        return Some(DetectedStereoFormat {
                            format: StereoFormat::Mono,
                            eye_order: EyeOrder::LeftRight,
                            projection: ProjectionType::FlatVirtualScreen,
                            detection_source: DetectionSource::ContainerMetadata,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

fn tokenize_filename(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn detect_projection_from_str(s: &str) -> ProjectionType {
    let lower = s.to_lowercase();
    let tokens = tokenize_filename(&lower);
    detect_projection_from_tokens(&tokens)
}

fn detect_projection_from_tokens(tokens: &[String]) -> ProjectionType {
    for tok in tokens {
        if tok == "180" || tok == "vr180" || tok.contains("180x180") {
            return ProjectionType::Vr180;
        }
        if tok == "360" || tok == "vr360" || tok.contains("360x180") {
            return ProjectionType::Vr360;
        }
        if tok == "fisheye" || tok == "fish-eye" {
            return ProjectionType::Fisheye;
        }
    }
    ProjectionType::FlatVirtualScreen
}

fn detect_eye_order_from_str(s: &str) -> Option<EyeOrder> {
    let lower = s.to_lowercase();
    let tokens = tokenize_filename(&lower);
    detect_eye_order_from_tokens(&tokens)
}

fn detect_eye_order_from_tokens(tokens: &[String]) -> Option<EyeOrder> {
    for tok in tokens {
        match tok.as_str() {
            "rl" | "_rl" => return Some(EyeOrder::RightLeft),
            "lr" | "_lr" => return Some(EyeOrder::LeftRight),
            "bt" | "_bt" => return Some(EyeOrder::BottomTop),
            "tb" | "_tb" => return Some(EyeOrder::TopBottom),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MediaStream;

    #[test]
    fn test_jellyfin_3d_format_precedence() {
        let item = MediaItem {
            id: "1".to_string(),
            name: "Avatar.2D.mp4".to_string(), // filename says 2D
            original_title: None,
            path: Some("/media/Avatar.2D.mp4".to_string()),
            video_3d_format: Some(JellyfinVideo3DFormat::HalfSideBySide),
            media_streams: vec![],
            container: Some("mp4".to_string()),
            run_time_ticks: None,
            overview: None,
            item_type: Some("Movie".to_string()),
            user_data: None,
        };

        let detected = detect_stereo_format(&item);
        assert_eq!(detected.format, StereoFormat::HalfSideBySide);
        assert_eq!(detected.detection_source, DetectionSource::Jellyfin3DFormat);
    }

    #[test]
    fn test_filename_tags_every_tag() {
        let cases = [
            (
                "Avatar (2009).3D.HSBS.mkv",
                StereoFormat::HalfSideBySide,
                EyeOrder::LeftRight,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Movie.FSBS.1080p.mkv",
                StereoFormat::FullSideBySide,
                EyeOrder::LeftRight,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Demo.HOU.mkv",
                StereoFormat::HalfOverUnder,
                EyeOrder::TopBottom,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Film.TAB.3D.mkv",
                StereoFormat::HalfOverUnder,
                EyeOrder::TopBottom,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Scene.OU.mp4",
                StereoFormat::HalfOverUnder,
                EyeOrder::TopBottom,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Rollercoaster.VR180.HSBS.mp4",
                StereoFormat::HalfSideBySide,
                EyeOrder::LeftRight,
                ProjectionType::Vr180,
            ),
            (
                "Space.360.FSBS.mp4",
                StereoFormat::FullSideBySide,
                EyeOrder::LeftRight,
                ProjectionType::Vr360,
            ),
            (
                "Nature.fisheye.SBS.mp4",
                StereoFormat::HalfSideBySide,
                EyeOrder::LeftRight,
                ProjectionType::Fisheye,
            ),
            (
                "Movie_3D_RL.mkv",
                StereoFormat::HalfSideBySide,
                EyeOrder::RightLeft,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Movie_TAB_BT.mkv",
                StereoFormat::HalfOverUnder,
                EyeOrder::BottomTop,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Film_3D_LR.mkv",
                StereoFormat::HalfSideBySide,
                EyeOrder::LeftRight,
                ProjectionType::FlatVirtualScreen,
            ),
            (
                "Film_TAB_TB.mkv",
                StereoFormat::HalfOverUnder,
                EyeOrder::TopBottom,
                ProjectionType::FlatVirtualScreen,
            ),
        ];

        for (filename, expected_format, expected_eye, expected_proj) in cases {
            let detected =
                detect_from_filename(filename).unwrap_or_else(|| panic!("Failed for {}", filename));
            assert_eq!(
                detected.format, expected_format,
                "Format mismatch for {}",
                filename
            );
            assert_eq!(
                detected.eye_order, expected_eye,
                "Eye order mismatch for {}",
                filename
            );
            assert_eq!(
                detected.projection, expected_proj,
                "Projection mismatch for {}",
                filename
            );
            assert_eq!(detected.detection_source, DetectionSource::FilenameTag);
        }
    }

    #[test]
    fn test_container_metadata_fallback() {
        let item = MediaItem {
            id: "2".to_string(),
            name: "PlainMovie.mkv".to_string(),
            original_title: None,
            path: Some("/media/PlainMovie.mkv".to_string()),
            video_3d_format: None,
            media_streams: vec![MediaStream {
                codec: Some("hevc".to_string()),
                stream_type: Some(StreamType::Video),
                stereo_mode: Some("1".to_string()),
                ..Default::default()
            }],
            container: Some("mkv".to_string()),
            run_time_ticks: None,
            overview: None,
            item_type: Some("Movie".to_string()),
            user_data: None,
        };

        let detected = detect_stereo_format(&item);
        assert_eq!(detected.format, StereoFormat::HalfSideBySide);
        assert_eq!(
            detected.detection_source,
            DetectionSource::ContainerMetadata
        );
    }

    #[test]
    fn test_plain_2d_fallback() {
        let item = MediaItem {
            id: "3".to_string(),
            name: "Normal2DMovie.mkv".to_string(),
            original_title: None,
            path: Some("/media/Normal2DMovie.mkv".to_string()),
            video_3d_format: None,
            media_streams: vec![],
            container: Some("mkv".to_string()),
            run_time_ticks: None,
            overview: None,
            item_type: Some("Movie".to_string()),
            user_data: None,
        };

        let detected = detect_stereo_format(&item);
        assert_eq!(detected.format, StereoFormat::Mono);
        assert_eq!(detected.detection_source, DetectionSource::FallbackDefault);
    }
}
