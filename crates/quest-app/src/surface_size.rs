//! Track actual decoded dimensions before changing an OpenXR surface.
use quest_jellyfin_player::{
    build_stereo_eye_layer_specs, calculate_display_aspect_ratio, EyeOrder, Projection,
    StereoFormat,
};

/// Parses JNI stereo format and eye-order strings for immersive playback.
///
/// Missing/empty format strings fall back to `StereoFormat::Mono`. Unrecognized eye orders
/// fall back to the format's natural default (`TopBottom` for OU, `LeftRight` otherwise).
pub fn parse_immersive_stereo(
    format: &str,
    order: &str,
) -> Result<(StereoFormat, EyeOrder), &'static str> {
    let format = match format.trim() {
        "" | "Mono" => StereoFormat::Mono,
        "HalfSideBySide" => StereoFormat::HalfSideBySide,
        "FullSideBySide" => StereoFormat::FullSideBySide,
        "HalfOverUnder" => StereoFormat::HalfOverUnder,
        "FullOverUnder" => StereoFormat::FullOverUnder,
        _ => return Err("This stereo format is not supported yet"),
    };
    let order = match format {
        StereoFormat::HalfOverUnder | StereoFormat::FullOverUnder => match order.trim() {
            "BottomTop" => EyeOrder::BottomTop,
            _ => EyeOrder::TopBottom,
        },
        _ => match order.trim() {
            "RightLeft" => EyeOrder::RightLeft,
            _ => EyeOrder::LeftRight,
        },
    };
    Ok((format, order))
}

/// Computes the flat screen dimensions used by the immersive renderer.
pub fn immersive_quad_size(
    screen_width: f32,
    format: StereoFormat,
    video_width: u32,
    video_height: u32,
) -> (f32, f32) {
    let aspect = calculate_display_aspect_ratio(format, video_width, video_height);
    (screen_width, screen_width / aspect)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl SurfaceSize {
    pub fn new(
        width: u32,
        height: u32,
        format: StereoFormat,
        order: EyeOrder,
    ) -> Result<Self, String> {
        if matches!(
            format,
            StereoFormat::Mvc | StereoFormat::MvHevc | StereoFormat::Unspecified3d
        ) {
            return Err(
                "Stereo format requires a supported two-view decoder or explicit packing".into(),
            );
        }
        // Mono's eye-layer helper does not check dimensions; validate them here too.
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err("Invalid decoded video dimensions".into());
        }
        build_stereo_eye_layer_specs(format, order, width, height).map_err(|e| e.to_string())?;
        Ok(Self { width, height })
    }
    pub fn requires_resize(self, width: u32, height: u32) -> bool {
        self.width != width || self.height != height
    }
}

/// Only implemented projections reach the compositor; never silently flatten fisheye.
pub fn parse_projection(value: &str) -> Result<Projection, &'static str> {
    match value.trim() {
        "" | "Flat" => Ok(Projection::Flat),
        "Equirectangular180" => Ok(Projection::Equirectangular180),
        "Equirectangular360" => Ok(Projection::Equirectangular360),
        _ => Err("This video projection is not supported yet"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_changed_even_stereo_frames_and_rejects_incomplete_eyes() {
        let size = SurfaceSize::new(
            1920,
            1080,
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
        )
        .unwrap();
        assert!(!size.requires_resize(1920, 1080));
        assert!(size.requires_resize(1280, 720));
        assert!(
            SurfaceSize::new(1279, 720, StereoFormat::HalfSideBySide, EyeOrder::LeftRight).is_err()
        );
        assert!(
            SurfaceSize::new(1280, 719, StereoFormat::FullOverUnder, EyeOrder::TopBottom).is_err()
        );
    }
    #[test]
    fn rejects_formats_that_cannot_be_cropped_into_two_views() {
        for format in [
            StereoFormat::Mvc,
            StereoFormat::MvHevc,
            StereoFormat::Unspecified3d,
        ] {
            assert!(SurfaceSize::new(1920, 1080, format, EyeOrder::LeftRight).is_err());
        }
    }
    #[test]
    fn rejects_empty_and_overflowing_mono_frames() {
        for (w, h) in [(0, 720), (1280, 0), (u32::MAX, 10)] {
            assert!(SurfaceSize::new(w, h, StereoFormat::Mono, EyeOrder::LeftRight).is_err());
        }
    }

    #[test]
    fn computes_immersive_quad_size_from_stereo_display_aspect() {
        let width = 4.0;
        for (format, video_width, video_height) in [
            (StereoFormat::Mono, 1920, 1080),
            (StereoFormat::HalfSideBySide, 1920, 1080),
            (StereoFormat::FullSideBySide, 3840, 1080),
            (StereoFormat::HalfOverUnder, 1920, 1080),
            (StereoFormat::FullOverUnder, 1920, 2160),
        ] {
            let (actual_width, actual_height) =
                immersive_quad_size(width, format, video_width, video_height);
            assert_eq!(actual_width, width);
            assert!((actual_height - 2.25).abs() < 1e-4, "{format:?}");
        }
    }

    #[test]
    fn only_supported_projections_reach_the_compositor() {
        assert_eq!(parse_projection("Flat"), Ok(Projection::Flat));
        assert_eq!(
            parse_projection("Equirectangular180"),
            Ok(Projection::Equirectangular180)
        );
        assert_eq!(
            parse_projection("Equirectangular360"),
            Ok(Projection::Equirectangular360)
        );
        for unsupported in ["Fisheye", "Unknown"] {
            assert!(parse_projection(unsupported).is_err());
        }
    }

    #[test]
    fn parses_immersive_stereo_with_mono_fallback_and_packing_default_orders() {
        assert_eq!(
            parse_immersive_stereo("", ""),
            Ok((StereoFormat::Mono, EyeOrder::LeftRight))
        );
        assert_eq!(
            parse_immersive_stereo("Mono", ""),
            Ok((StereoFormat::Mono, EyeOrder::LeftRight))
        );
        assert_eq!(
            parse_immersive_stereo("HalfSideBySide", "RightLeft"),
            Ok((StereoFormat::HalfSideBySide, EyeOrder::RightLeft))
        );
        assert_eq!(
            parse_immersive_stereo("HalfOverUnder", ""),
            Ok((StereoFormat::HalfOverUnder, EyeOrder::TopBottom))
        );
        assert_eq!(
            parse_immersive_stereo("FullOverUnder", "LeftRight"),
            Ok((StereoFormat::FullOverUnder, EyeOrder::TopBottom))
        );
        assert_eq!(
            parse_immersive_stereo("FullOverUnder", "BottomTop"),
            Ok((StereoFormat::FullOverUnder, EyeOrder::BottomTop))
        );
        for unsupported in ["Mvc", "MvHevc", "Unspecified3d", "Unknown"] {
            assert!(parse_immersive_stereo(unsupported, "").is_err());
        }
    }
}
