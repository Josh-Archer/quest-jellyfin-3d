//! Stereo format for a Jellyfin item.
//!
//! Packing and projection come from `xr_video::format` (DeskQuest#220). This
//! module only adds what xr-video does not model: which half holds which eye.

use jellyfin_client::{BaseItem, Video3DFormat};
use xr_video::format::{detect_format, ContainerHints, DetectionSource, Projection};

pub use xr_video::format::StereoFormat;

/// Which half of a packed frame holds the left eye.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EyeOrder {
    /// Left eye in the left half (side by side). Default.
    #[default]
    LeftRight,
    /// Left eye in the right half (cross-eyed side by side).
    RightLeft,
    /// Left eye in the top half (over/under).
    TopBottom,
    /// Left eye in the bottom half (over/under).
    BottomTop,
}

/// Packing, eye order, and projection for one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectedStereoFormat {
    pub format: StereoFormat,
    pub eye_order: EyeOrder,
    pub projection: Projection,
    pub source: DetectionSource,
}

/// Jellyfin's `Video3DFormat` wins; otherwise the filename is inspected.
///
/// Main's `Video3DFormat` is mapped here rather than through
/// `xr_video::format::jellyfin_stereo`, because Jellyfin spells over/under as
/// `FullTopAndBottom` / `HalfTopAndBottom`.
pub fn detect_item_format(item: &BaseItem) -> DetectedStereoFormat {
    let detected = detect_format(None, item.path.as_deref(), ContainerHints::default());
    let (format, source) = match item.video_3d_format.as_ref().and_then(jellyfin_stereo) {
        Some(format) => (format, DetectionSource::Jellyfin),
        None => (detected.stereo, detected.stereo_source),
    };
    let from_name = item.path.as_deref().and_then(eye_order_from_filename);
    let eye_order = match format {
        StereoFormat::HalfOverUnder | StereoFormat::FullOverUnder => match from_name {
            Some(EyeOrder::BottomTop) => EyeOrder::BottomTop,
            _ => EyeOrder::TopBottom,
        },
        StereoFormat::HalfSideBySide | StereoFormat::FullSideBySide => match from_name {
            Some(EyeOrder::RightLeft) => EyeOrder::RightLeft,
            _ => EyeOrder::LeftRight,
        },
        _ => EyeOrder::LeftRight,
    };
    DetectedStereoFormat {
        format,
        eye_order,
        projection: detected.projection,
        source,
    }
}

fn jellyfin_stereo(value: &Video3DFormat) -> Option<StereoFormat> {
    match value {
        Video3DFormat::HalfSideBySide => Some(StereoFormat::HalfSideBySide),
        Video3DFormat::FullSideBySide => Some(StereoFormat::FullSideBySide),
        Video3DFormat::HalfTopAndBottom => Some(StereoFormat::HalfOverUnder),
        Video3DFormat::FullTopAndBottom => Some(StereoFormat::FullOverUnder),
        Video3DFormat::Mvc => Some(StereoFormat::Mvc),
        Video3DFormat::Other(other) => xr_video::format::jellyfin_stereo(other),
    }
}

/// `RL`/`BT` (and `LR`/`TB`) as whole tokens of the basename.
fn eye_order_from_filename(path: &str) -> Option<EyeOrder> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.split(|c: char| !c.is_ascii_alphanumeric())
        .find_map(|token| match token.to_ascii_lowercase().as_str() {
            "rl" => Some(EyeOrder::RightLeft),
            "lr" => Some(EyeOrder::LeftRight),
            "bt" => Some(EyeOrder::BottomTop),
            "tb" => Some(EyeOrder::TopBottom),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(fmt: Option<Video3DFormat>, path: &str) -> BaseItem {
        BaseItem {
            video_3d_format: fmt,
            path: Some(path.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn jellyfin_metadata_wins_over_filename() {
        let d = detect_item_format(&item(
            Some(Video3DFormat::FullTopAndBottom),
            "/m/Movie.SBS.mkv",
        ));
        assert_eq!(d.format, StereoFormat::FullOverUnder);
        assert_eq!(d.eye_order, EyeOrder::TopBottom);
        assert_eq!(d.source, DetectionSource::Jellyfin);
    }

    #[test]
    fn filename_packing_and_eye_order() {
        let d = detect_item_format(&item(None, "/m/Movie_3D_SBS_RL.mkv"));
        assert_eq!(d.format, StereoFormat::HalfSideBySide);
        assert_eq!(d.eye_order, EyeOrder::RightLeft);
        assert_eq!(d.source, DetectionSource::Filename);

        let d = detect_item_format(&item(
            Some(Video3DFormat::HalfTopAndBottom),
            "/m/Movie_BT.mkv",
        ));
        assert_eq!(d.eye_order, EyeOrder::BottomTop);
    }

    #[test]
    fn mismatched_eye_order_falls_back_to_the_packing_default() {
        let d = detect_item_format(&item(
            Some(Video3DFormat::HalfSideBySide),
            "/m/Movie_BT.mkv",
        ));
        assert_eq!(d.eye_order, EyeOrder::LeftRight);
    }

    #[test]
    fn plain_movie_is_mono_flat() {
        let d = detect_item_format(&item(None, "/m/Movie (2020) 1080p.mkv"));
        assert_eq!(d.format, StereoFormat::Mono);
        assert_eq!(d.projection, Projection::Flat);
        assert_eq!(d.source, DetectionSource::Default);
    }
}
