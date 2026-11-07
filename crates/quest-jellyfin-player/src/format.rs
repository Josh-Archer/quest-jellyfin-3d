//! Stereo format for a Jellyfin item.
//!
//! Packing and projection come from `xr_video::format` (DeskQuest#220). This
//! module only adds what xr-video does not model: which half holds which eye.

use jellyfin_client::{BaseItem, Video3DFormat};
use xr_video::format::{detect_format, ContainerHints, DetectionSource};

pub use xr_video::format::{Projection, StereoFormat};

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
    let explicit_multiview = item.media_streams.iter().any(|stream| {
        stream.codec.as_deref().is_some_and(is_mv_hevc_label)
            || stream.profile.as_deref().is_some_and(is_mv_hevc_label)
    });
    let filename_multiview = item.path.as_deref().is_some_and(is_mv_hevc_filename);
    let (format, source) = if explicit_multiview {
        (StereoFormat::MvHevc, DetectionSource::Container)
    } else if filename_multiview {
        (StereoFormat::MvHevc, DetectionSource::Filename)
    } else {
        match item.video_3d_format.as_ref().and_then(jellyfin_stereo) {
            Some(format) => (format, DetectionSource::Jellyfin),
            None => (detected.stereo, detected.stereo_source),
        }
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
        Video3DFormat::Other(other) if is_mv_hevc_label(other) => Some(StereoFormat::MvHevc),
        Video3DFormat::Other(other) => xr_video::format::jellyfin_stereo(other),
    }
}

fn is_mv_hevc_label(value: &str) -> bool {
    let normalized: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "mvhevc" | "spatialmvhevc" | "multiviewhevc" | "multiviewmain" | "multiviewmain10"
    )
}

fn is_mv_hevc_filename(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let tokens: Vec<_> = stem
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    tokens.iter().any(|token| is_mv_hevc_label(token))
        || tokens
            .windows(2)
            .any(|pair| pair[0].eq_ignore_ascii_case("mv") && pair[1].eq_ignore_ascii_case("hevc"))
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
    fn multiview_is_never_mistaken_for_ordinary_hevc_or_sbs() {
        use jellyfin_client::MediaStream;
        for label in ["MV-HEVC", "SpatialMvHevc", "Multiview Main 10"] {
            let mut video = item(Some(Video3DFormat::HalfSideBySide), "/m/video.mkv");
            video.media_streams = vec![MediaStream {
                codec: Some(label.into()),
                ..Default::default()
            }];
            assert_eq!(
                detect_item_format(&video).format,
                StereoFormat::MvHevc,
                "{label}"
            );
        }
        assert_eq!(
            detect_item_format(&item(None, "/m/video.MV-HEVC.mkv")).format,
            StereoFormat::MvHevc
        );
        assert_eq!(
            detect_item_format(&item(
                Some(Video3DFormat::Other("SpatialMvHevc".into())),
                "/m/video.mkv"
            ))
            .format,
            StereoFormat::MvHevc
        );
        let mut regular = item(None, "/m/video.hevc.mkv");
        regular.media_streams = vec![MediaStream {
            codec: Some("hevc".into()),
            profile: Some("Main 10".into()),
            ..Default::default()
        }];
        assert_eq!(detect_item_format(&regular).format, StereoFormat::Mono);
    }

    #[test]
    fn plain_movie_is_mono_flat() {
        let d = detect_item_format(&item(None, "/m/Movie (2020) 1080p.mkv"));
        assert_eq!(d.format, StereoFormat::Mono);
        assert_eq!(d.projection, Projection::Flat);
        assert_eq!(d.source, DetectionSource::Default);
    }
}
