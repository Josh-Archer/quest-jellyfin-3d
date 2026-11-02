//! Stereoscopic layout partitioning, UV coordinate math, and aspect ratio computation.

use crate::error::{PlayerError, Result};
use crate::format::{EyeOrder, StereoFormat};
use serde::{Deserialize, Serialize};
pub use xr_video::stereo::{Eye, EyeRegion, StereoLayout};

/// Normalized texture UV coordinates spanning [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedUvRect {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

impl NormalizedUvRect {
    pub const FULL_FRAME: Self = Self {
        u_min: 0.0,
        v_min: 0.0,
        u_max: 1.0,
        v_max: 1.0,
    };

    /// Texture UV offset `(u_offset, v_offset)` for the top-left corner of the eye crop.
    pub const fn offset(self) -> (f32, f32) {
        (self.u_min, self.v_min)
    }

    /// Texture UV scale `(u_scale, v_scale)` across the normalized `[0.0, 1.0]` eye span.
    pub fn scale(self) -> (f32, f32) {
        (self.u_max - self.u_min, self.v_max - self.v_min)
    }

    /// Maps normalized virtual-screen UV `(u, v)` in `[0.0, 1.0]^2` into frame texture UV.
    pub fn apply(self, u: f32, v: f32) -> (f32, f32) {
        let (scale_u, scale_v) = self.scale();
        (self.u_min + u * scale_u, self.v_min + v * scale_v)
    }
}

/// Per-eye UV crop offset and scale mapping local `[0.0, 1.0]^2` virtual-screen UVs into the
/// decoded video texture, paired with the anamorphic aspect-ratio scale correction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EyeUvCrop {
    /// Normalized texture origin `(u_offset, v_offset)` for this eye.
    pub offset: (f32, f32),
    /// Normalized texture span `(u_scale, v_scale)` sampled across `[0.0, 1.0]^2`.
    pub scale: (f32, f32),
    /// Anamorphic unsqueeze factor `(width_scale, height_scale)` applied to the cropped eye view.
    pub aspect_scale: (f32, f32),
}

impl EyeUvCrop {
    pub const FULL_FRAME: Self = Self {
        offset: (0.0, 0.0),
        scale: (1.0, 1.0),
        aspect_scale: (1.0, 1.0),
    };

    /// Maps normalized virtual-screen UV `(u, v)` into the decoded frame's texture UV space.
    pub fn apply(self, u: f32, v: f32) -> (f32, f32) {
        (
            self.offset.0 + u * self.scale.0,
            self.offset.1 + v * self.scale.1,
        )
    }

    /// Converts crop offset and scale into `[u_min, v_min, u_max, v_max]` UV bounds.
    pub fn to_uv_rect(self) -> NormalizedUvRect {
        NormalizedUvRect {
            u_min: self.offset.0,
            v_min: self.offset.1,
            u_max: self.offset.0 + self.scale.0,
            v_max: self.offset.1 + self.scale.1,
        }
    }
}

/// Pixel rectangle within the decoded frame texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StereoPixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl From<StereoPixelRect> for EyeRegion {
    fn from(r: StereoPixelRect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<EyeRegion> for StereoPixelRect {
    fn from(r: EyeRegion) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Returns true when `format` packs left and right eye views into a single video frame.
pub const fn is_packed_stereo(format: StereoFormat) -> bool {
    matches!(
        format,
        StereoFormat::HalfSideBySide
            | StereoFormat::FullSideBySide
            | StereoFormat::HalfOverUnder
            | StereoFormat::FullOverUnder
    )
}

/// Maps a [`StereoFormat`] onto `xr-video`'s frame-partitioning [`StereoLayout`].
///
/// Unpacked formats (`Mono`, `Mvc`, `MvHevc`, `Unspecified3d`) fall back to `StereoLayout::Mono`.
pub const fn stereo_layout(format: StereoFormat) -> StereoLayout {
    match format {
        StereoFormat::HalfSideBySide | StereoFormat::FullSideBySide => StereoLayout::SideBySide,
        StereoFormat::HalfOverUnder | StereoFormat::FullOverUnder => StereoLayout::OverUnder,
        StereoFormat::Mono
        | StereoFormat::Mvc
        | StereoFormat::MvHevc
        | StereoFormat::Unspecified3d => StereoLayout::Mono,
    }
}

/// Returns the per-axis anamorphic unsqueeze factor `(width_scale, height_scale)` for a cropped eye view.
///
/// Half-SBS frames squeeze each eye 2x horizontally; half-OU frames squeeze each eye 2x vertically.
/// Full-SBS, full-OU, and mono frames store unsqueezed pixels `(1.0, 1.0)`.
pub const fn aspect_correction_scale(format: StereoFormat) -> (f32, f32) {
    match format {
        StereoFormat::HalfSideBySide => (2.0, 1.0),
        StereoFormat::HalfOverUnder => (1.0, 2.0),
        StereoFormat::Mono
        | StereoFormat::FullSideBySide
        | StereoFormat::FullOverUnder
        | StereoFormat::Mvc
        | StereoFormat::MvHevc
        | StereoFormat::Unspecified3d => (1.0, 1.0),
    }
}

/// Resolves which half (`Eye::Left` = left/top, `Eye::Right` = right/bottom) a viewer eye samples.
///
/// Mismatched `EyeOrder` values (such as `EyeOrder::LeftRight` on an over-under item) fall back to
/// the format's standard ordering (`LeftRight` for SBS, `TopBottom` for OU) so both eyes never
/// collapse onto the same half.
const fn effective_layout_eye(format: StereoFormat, eye_order: EyeOrder, eye: Eye) -> Eye {
    let swapped = match format {
        StereoFormat::HalfSideBySide | StereoFormat::FullSideBySide => {
            matches!(eye_order, EyeOrder::RightLeft)
        }
        StereoFormat::HalfOverUnder | StereoFormat::FullOverUnder => {
            matches!(eye_order, EyeOrder::BottomTop)
        }
        StereoFormat::Mono
        | StereoFormat::Mvc
        | StereoFormat::MvHevc
        | StereoFormat::Unspecified3d => false,
    };

    if swapped {
        match eye {
            Eye::Left => Eye::Right,
            Eye::Right => Eye::Left,
        }
    } else {
        eye
    }
}

/// Calculates the per-eye UV crop offset, UV scale, and anamorphic aspect correction.
///
/// Mono and unpacked formats fall back to full-frame UV `(offset = (0, 0), scale = (1, 1))` for both eyes.
pub const fn calculate_eye_uv_crop(
    format: StereoFormat,
    eye_order: EyeOrder,
    eye: Eye,
) -> EyeUvCrop {
    let layout = stereo_layout(format);
    let layout_eye = effective_layout_eye(format, eye_order, eye);
    let aspect_scale = aspect_correction_scale(format);

    match layout {
        StereoLayout::Mono => EyeUvCrop::FULL_FRAME,
        StereoLayout::SideBySide => EyeUvCrop {
            offset: (
                match layout_eye {
                    Eye::Left => 0.0,
                    Eye::Right => 0.5,
                },
                0.0,
            ),
            scale: (0.5, 1.0),
            aspect_scale,
        },
        StereoLayout::OverUnder => EyeUvCrop {
            offset: (
                0.0,
                match layout_eye {
                    Eye::Left => 0.0,
                    Eye::Right => 0.5,
                },
            ),
            scale: (1.0, 0.5),
            aspect_scale,
        },
    }
}

/// Calculates the normalized UV coordinates for the specified eye based on format and eye ordering.
pub fn calculate_eye_uv(format: StereoFormat, eye_order: EyeOrder, eye: Eye) -> NormalizedUvRect {
    calculate_eye_uv_crop(format, eye_order, eye).to_uv_rect()
}

/// Calculates pixel sub-image rectangle within the video frame for composition layers,
/// delegating frame bounds and even-dimension checks to `xr_video::stereo::StereoLayout`.
pub fn calculate_eye_pixel_rect(
    format: StereoFormat,
    eye_order: EyeOrder,
    eye: Eye,
    width: u32,
    height: u32,
) -> Result<StereoPixelRect> {
    let layout = stereo_layout(format);
    let layout_eye = effective_layout_eye(format, eye_order, eye);
    layout
        .eye_region(layout_eye, width, height)
        .map(StereoPixelRect::from)
        .map_err(|e| PlayerError::StereoLayout(e.to_string()))
}

/// Computes the correct physical display aspect ratio (width / height) on the virtual screen.
///
/// Anamorphic layout rules:
/// - **Half SBS**: A 1920x1080 frame stores two 960x1080 eye images squeezed horizontally by 2x.
///   Each eye must be scaled by 2x in width to restore the intended aspect ratio:
///   `aspect = (pixel_w / 2) * 2 / pixel_h = pixel_w / pixel_h` (e.g. 1920/1080 ≈ 1.777)
/// - **Full SBS**: A 3840x1080 frame stores two unsqueezed 1920x1080 eye images side-by-side:
///   `aspect = (pixel_w / 2) / pixel_h = 1920 / 1080 ≈ 1.777`
/// - **Half OU**: A 1920x1080 frame stores two 1920x540 eye images squeezed vertically by 2x.
///   Each eye must be scaled by 2x in height to restore the intended aspect ratio:
///   `aspect = pixel_w / ((pixel_h / 2) * 2) = pixel_w / pixel_h` (e.g. 1920/1080 ≈ 1.777)
/// - **Full OU**: A 1920x2160 frame stores two unsqueezed 1920x1080 eye images vertically:
///   `aspect = pixel_w / (pixel_h / 2) = 1920 / 1080 ≈ 1.777`
/// - **Mono**: `pixel_w / pixel_h`
pub fn calculate_display_aspect_ratio(format: StereoFormat, width: u32, height: u32) -> f32 {
    if width == 0 || height == 0 {
        return 16.0 / 9.0;
    }

    let crop = calculate_eye_uv_crop(format, EyeOrder::LeftRight, Eye::Left);
    let eye_width = width as f32 * crop.scale.0 * crop.aspect_scale.0;
    let eye_height = height as f32 * crop.scale.1 * crop.aspect_scale.1;
    eye_width / eye_height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uv_half_and_full_sbs() {
        let left = calculate_eye_uv(StereoFormat::HalfSideBySide, EyeOrder::LeftRight, Eye::Left);
        let right = calculate_eye_uv(
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            Eye::Right,
        );

        assert_eq!(
            left,
            NormalizedUvRect {
                u_min: 0.0,
                v_min: 0.0,
                u_max: 0.5,
                v_max: 1.0,
            }
        );
        assert_eq!(
            right,
            NormalizedUvRect {
                u_min: 0.5,
                v_min: 0.0,
                u_max: 1.0,
                v_max: 1.0,
            }
        );
    }

    #[test]
    fn test_uv_sbs_swapped_eye_order() {
        let left = calculate_eye_uv(StereoFormat::FullSideBySide, EyeOrder::RightLeft, Eye::Left);
        let right = calculate_eye_uv(
            StereoFormat::FullSideBySide,
            EyeOrder::RightLeft,
            Eye::Right,
        );

        // When reversed, Left eye gets [0.5, 1.0] and Right eye gets [0.0, 0.5]
        assert_eq!(left.u_min, 0.5);
        assert_eq!(left.u_max, 1.0);
        assert_eq!(right.u_min, 0.0);
        assert_eq!(right.u_max, 0.5);
    }

    #[test]
    fn test_uv_over_under() {
        let left = calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::TopBottom, Eye::Left);
        let right = calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::TopBottom, Eye::Right);

        assert_eq!(
            left,
            NormalizedUvRect {
                u_min: 0.0,
                v_min: 0.0,
                u_max: 1.0,
                v_max: 0.5,
            }
        );
        assert_eq!(
            right,
            NormalizedUvRect {
                u_min: 0.0,
                v_min: 0.5,
                u_max: 1.0,
                v_max: 1.0,
            }
        );
    }

    #[test]
    fn test_pixel_rect_partitioning() {
        // Full SBS 3840x1080
        let left = calculate_eye_pixel_rect(
            StereoFormat::FullSideBySide,
            EyeOrder::LeftRight,
            Eye::Left,
            3840,
            1080,
        )
        .unwrap();
        let right = calculate_eye_pixel_rect(
            StereoFormat::FullSideBySide,
            EyeOrder::LeftRight,
            Eye::Right,
            3840,
            1080,
        )
        .unwrap();

        assert_eq!(
            left,
            StereoPixelRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
        assert_eq!(
            right,
            StereoPixelRect {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080
            }
        );

        // Full OU 1920x2160
        let top = calculate_eye_pixel_rect(
            StereoFormat::FullOverUnder,
            EyeOrder::TopBottom,
            Eye::Left,
            1920,
            2160,
        )
        .unwrap();
        let bottom = calculate_eye_pixel_rect(
            StereoFormat::FullOverUnder,
            EyeOrder::TopBottom,
            Eye::Right,
            1920,
            2160,
        )
        .unwrap();

        assert_eq!(
            top,
            StereoPixelRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
        assert_eq!(
            bottom,
            StereoPixelRect {
                x: 0,
                y: 1080,
                width: 1920,
                height: 1080
            }
        );
    }

    #[test]
    fn test_display_aspect_ratios() {
        let eps = 1e-4;

        // 1080p Half SBS: 1920x1080 -> 16:9
        let hsbs_aspect = calculate_display_aspect_ratio(StereoFormat::HalfSideBySide, 1920, 1080);
        assert!((hsbs_aspect - 16.0 / 9.0).abs() < eps);

        // Full SBS: 3840x1080 -> 16:9
        let fsbs_aspect = calculate_display_aspect_ratio(StereoFormat::FullSideBySide, 3840, 1080);
        assert!((fsbs_aspect - 16.0 / 9.0).abs() < eps);

        // 1080p Half OU: 1920x1080 -> 16:9
        let hou_aspect = calculate_display_aspect_ratio(StereoFormat::HalfOverUnder, 1920, 1080);
        assert!((hou_aspect - 16.0 / 9.0).abs() < eps);

        // Full OU: 1920x2160 -> 16:9
        let fou_aspect = calculate_display_aspect_ratio(StereoFormat::FullOverUnder, 1920, 2160);
        assert!((fou_aspect - 16.0 / 9.0).abs() < eps);

        // Zero width or height falls back to 16:9 rather than 0.0 or infinity
        assert!(
            (calculate_display_aspect_ratio(StereoFormat::Mono, 0, 1080) - 16.0 / 9.0).abs() < eps
        );
        assert!(
            (calculate_display_aspect_ratio(StereoFormat::Mono, 1920, 0) - 16.0 / 9.0).abs() < eps
        );
    }

    #[test]
    fn test_uv_crop_scale_and_aspect_correction() {
        let hsbs_left =
            calculate_eye_uv_crop(StereoFormat::HalfSideBySide, EyeOrder::LeftRight, Eye::Left);
        let hsbs_right = calculate_eye_uv_crop(
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            Eye::Right,
        );
        assert_eq!(hsbs_left.offset, (0.0, 0.0));
        assert_eq!(hsbs_left.scale, (0.5, 1.0));
        assert_eq!(hsbs_left.aspect_scale, (2.0, 1.0));
        assert_eq!(hsbs_left.apply(0.5, 0.5), (0.25, 0.5));
        assert_eq!(hsbs_right.offset, (0.5, 0.0));
        assert_eq!(hsbs_right.scale, (0.5, 1.0));
        assert_eq!(hsbs_right.aspect_scale, (2.0, 1.0));
        assert_eq!(hsbs_right.apply(0.5, 0.5), (0.75, 0.5));

        let fsbs_left =
            calculate_eye_uv_crop(StereoFormat::FullSideBySide, EyeOrder::LeftRight, Eye::Left);
        assert_eq!(fsbs_left.scale, (0.5, 1.0));
        assert_eq!(fsbs_left.aspect_scale, (1.0, 1.0));

        let hou_left =
            calculate_eye_uv_crop(StereoFormat::HalfOverUnder, EyeOrder::TopBottom, Eye::Left);
        let hou_right =
            calculate_eye_uv_crop(StereoFormat::HalfOverUnder, EyeOrder::TopBottom, Eye::Right);
        assert_eq!(hou_left.offset, (0.0, 0.0));
        assert_eq!(hou_left.scale, (1.0, 0.5));
        assert_eq!(hou_left.aspect_scale, (1.0, 2.0));
        assert_eq!(hou_left.apply(0.5, 0.5), (0.5, 0.25));
        assert_eq!(hou_right.offset, (0.0, 0.5));
        assert_eq!(hou_right.scale, (1.0, 0.5));
        assert_eq!(hou_right.aspect_scale, (1.0, 2.0));
        assert_eq!(hou_right.apply(0.5, 0.5), (0.5, 0.75));

        let fou_left =
            calculate_eye_uv_crop(StereoFormat::FullOverUnder, EyeOrder::TopBottom, Eye::Left);
        assert_eq!(fou_left.scale, (1.0, 0.5));
        assert_eq!(fou_left.aspect_scale, (1.0, 1.0));

        // Mono and unpacked fallbacks map [0, 1]^2 identically for both eyes
        for format in [StereoFormat::Mono, StereoFormat::Unspecified3d] {
            for eye in [Eye::Left, Eye::Right] {
                let crop = calculate_eye_uv_crop(format, EyeOrder::LeftRight, eye);
                assert_eq!(crop, EyeUvCrop::FULL_FRAME);
                assert_eq!(crop.apply(0.25, 0.75), (0.25, 0.75));
            }
        }
    }

    #[test]
    fn test_mismatched_eye_order_never_collapses_both_eyes_to_one_half() {
        // EyeOrder::default() is LeftRight; passing it to an OU video must still partition top/bottom
        let ou_left = calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::default(), Eye::Left);
        let ou_right =
            calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::default(), Eye::Right);
        assert_eq!(ou_left.offset(), (0.0, 0.0));
        assert_eq!(ou_left.scale(), (1.0, 0.5));
        assert_eq!(ou_right.offset(), (0.0, 0.5));
        assert_eq!(ou_right.scale(), (1.0, 0.5));

        let sbs_left =
            calculate_eye_uv(StereoFormat::HalfSideBySide, EyeOrder::TopBottom, Eye::Left);
        let sbs_right = calculate_eye_uv(
            StereoFormat::HalfSideBySide,
            EyeOrder::TopBottom,
            Eye::Right,
        );
        assert_eq!(sbs_left.offset(), (0.0, 0.0));
        assert_eq!(sbs_right.offset(), (0.5, 0.0));
    }
}
