//! Stereoscopic layout partitioning, UV coordinate math, and aspect ratio computation.

use crate::error::{PlayerError, Result};
use crate::format::{EyeOrder, StereoFormat};
use serde::{Deserialize, Serialize};
pub use xr_video::stereo::{Eye, EyeRegion};

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

/// Calculates the normalized UV coordinates for the specified eye based on format and eye ordering.
pub fn calculate_eye_uv(format: StereoFormat, eye_order: EyeOrder, eye: Eye) -> NormalizedUvRect {
    match format {
        StereoFormat::Mono => NormalizedUvRect::FULL_FRAME,

        StereoFormat::HalfSideBySide | StereoFormat::FullSideBySide => {
            // Determine whether this eye is in the first (left) or second (right) half
            let is_first_half = matches!(
                (eye, eye_order),
                (Eye::Left, EyeOrder::LeftRight) | (Eye::Right, EyeOrder::RightLeft)
            );

            if is_first_half {
                NormalizedUvRect {
                    u_min: 0.0,
                    v_min: 0.0,
                    u_max: 0.5,
                    v_max: 1.0,
                }
            } else {
                NormalizedUvRect {
                    u_min: 0.5,
                    v_min: 0.0,
                    u_max: 1.0,
                    v_max: 1.0,
                }
            }
        }

        StereoFormat::HalfOverUnder | StereoFormat::FullOverUnder => {
            // Determine whether this eye is in the top or bottom half
            let is_top_half = matches!(
                (eye, eye_order),
                (Eye::Left, EyeOrder::TopBottom) | (Eye::Right, EyeOrder::BottomTop)
            );

            if is_top_half {
                NormalizedUvRect {
                    u_min: 0.0,
                    v_min: 0.0,
                    u_max: 1.0,
                    v_max: 0.5,
                }
            } else {
                NormalizedUvRect {
                    u_min: 0.0,
                    v_min: 0.5,
                    u_max: 1.0,
                    v_max: 1.0,
                }
            }
        }

        StereoFormat::Mvc | StereoFormat::MvHevc | StereoFormat::Unspecified3d => {
            // Unknown packing renders as mono; MVC decodes to independent full views; primary view displays full frame
            NormalizedUvRect::FULL_FRAME
        }
    }
}

/// Calculates pixel sub-image rectangle within the video frame for composition layers.
pub fn calculate_eye_pixel_rect(
    format: StereoFormat,
    eye_order: EyeOrder,
    eye: Eye,
    width: u32,
    height: u32,
) -> Result<StereoPixelRect> {
    if width == 0 || height == 0 {
        return Err(PlayerError::StereoLayout(
            "Video texture dimensions must be non-zero".to_string(),
        ));
    }

    match format {
        StereoFormat::Mono
        | StereoFormat::Mvc
        | StereoFormat::MvHevc
        | StereoFormat::Unspecified3d => Ok(StereoPixelRect {
            x: 0,
            y: 0,
            width,
            height,
        }),

        StereoFormat::HalfSideBySide | StereoFormat::FullSideBySide => {
            if !width.is_multiple_of(2) {
                return Err(PlayerError::StereoLayout(format!(
                    "Side-by-side video width must be even (got {})",
                    width
                )));
            }
            let half_w = width / 2;
            let is_first = matches!(
                (eye, eye_order),
                (Eye::Left, EyeOrder::LeftRight) | (Eye::Right, EyeOrder::RightLeft)
            );

            Ok(StereoPixelRect {
                x: if is_first { 0 } else { half_w },
                y: 0,
                width: half_w,
                height,
            })
        }

        StereoFormat::HalfOverUnder | StereoFormat::FullOverUnder => {
            if !height.is_multiple_of(2) {
                return Err(PlayerError::StereoLayout(format!(
                    "Over-under video height must be even (got {})",
                    height
                )));
            }
            let half_h = height / 2;
            let is_top = matches!(
                (eye, eye_order),
                (Eye::Left, EyeOrder::TopBottom) | (Eye::Right, EyeOrder::BottomTop)
            );

            Ok(StereoPixelRect {
                x: 0,
                y: if is_top { 0 } else { half_h },
                width,
                height: half_h,
            })
        }
    }
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
    if height == 0 {
        return 16.0 / 9.0;
    }

    let w = width as f32;
    let h = height as f32;

    match format {
        StereoFormat::Mono
        | StereoFormat::Mvc
        | StereoFormat::MvHevc
        | StereoFormat::Unspecified3d => w / h,
        StereoFormat::HalfSideBySide => w / h,
        StereoFormat::FullSideBySide => (w / 2.0) / h,
        StereoFormat::HalfOverUnder => w / h,
        StereoFormat::FullOverUnder => w / (h / 2.0),
    }
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
    }
}
