//! Virtual screen placement, environment mode (passthrough vs dark room), and OpenXR stereo layers.

use crate::error::Result;
use crate::format::{DetectedStereoFormat, EyeOrder, StereoFormat};
use crate::stereo::{
    calculate_display_aspect_ratio, calculate_eye_pixel_rect, calculate_eye_uv_crop,
    is_packed_stereo, Eye, EyeUvCrop, StereoPixelRect,
};
use openxr::{
    CompositionLayerCylinderKHR, CompositionLayerFlags, CompositionLayerQuad, EnvironmentBlendMode,
    Extent2Df, Extent2Di, EyeVisibility, Graphics, Offset2Di, Posef, Quaternionf, Rect2Di, Space,
    Swapchain, SwapchainSubImage, Vector3f,
};
use serde::{Deserialize, Serialize};
use xr_core::{DesktopLayerConfig, XrConfig};

/// Virtual environment viewing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VirtualScreenEnvironment {
    /// Passthrough mode: mixes the virtual cinema screen with real-world camera feed.
    #[default]
    Passthrough,
    /// Dark Room mode: complete blackout / dark void for theatrical viewing experience.
    DarkRoom,
}

impl VirtualScreenEnvironment {
    /// Maps a boolean passthrough preference to [`VirtualScreenEnvironment`].
    pub const fn from_passthrough(passthrough: bool) -> Self {
        if passthrough {
            Self::Passthrough
        } else {
            Self::DarkRoom
        }
    }

    /// Whether the OpenXR passthrough reconstruction backdrop is active.
    pub const fn uses_passthrough(self) -> bool {
        matches!(self, Self::Passthrough)
    }

    /// Selects the OpenXR environment blend mode (`ALPHA_BLEND` for passthrough when supported,
    /// falling back to `OPAQUE` for dark room or runtimes without alpha-blend support).
    pub const fn blend_mode(self, alpha_blend_supported: bool) -> EnvironmentBlendMode {
        if self.uses_passthrough() && alpha_blend_supported {
            EnvironmentBlendMode::ALPHA_BLEND
        } else {
            EnvironmentBlendMode::OPAQUE
        }
    }
}

/// Geometry shape of the virtual screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VirtualScreenShape {
    /// Rectilinear flat virtual screen quad.
    FlatQuad,
    /// Ergonomic curved cinema cylinder arc.
    #[default]
    CurvedCylinder,
}

/// Configuration defining the virtual screen display in 3D space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VirtualScreenConfig {
    /// Environment background mode (passthrough or dark cinema room).
    pub environment: VirtualScreenEnvironment,
    /// Display geometry mode (flat or curved).
    pub shape: VirtualScreenShape,
    /// Distance from headset origin in meters (defaults to 2.2m).
    pub distance_m: f32,
    /// Screen physical width in meters (defaults to 4.0m).
    pub screen_width_m: f32,
    /// Eye height elevation in meters (defaults to 1.4m in stage space).
    pub elevation_m: f32,
    /// Central arc angle for curved cylinder in radians (defaults to 1.2 rad ≈ 68.8°).
    pub cylinder_arc_rad: f32,
}

impl Default for VirtualScreenConfig {
    fn default() -> Self {
        Self {
            environment: VirtualScreenEnvironment::Passthrough,
            shape: VirtualScreenShape::CurvedCylinder,
            distance_m: 2.2,
            screen_width_m: 4.0,
            elevation_m: 1.4,
            cylinder_arc_rad: 1.2,
        }
    }
}

impl VirtualScreenConfig {
    /// Computes the physical screen height in meters based on the display aspect ratio.
    pub fn compute_screen_height_m(&self, display_aspect_ratio: f32) -> f32 {
        if !display_aspect_ratio.is_finite() || display_aspect_ratio <= 0.0 {
            self.screen_width_m / (16.0 / 9.0)
        } else {
            self.screen_width_m / display_aspect_ratio
        }
    }

    /// Converts configuration to `xr_core::DesktopLayerConfig`.
    pub fn to_desktop_layer_config(&self, display_aspect_ratio: f32) -> DesktopLayerConfig {
        let height_m = self.compute_screen_height_m(display_aspect_ratio);
        let mode = match self.shape {
            VirtualScreenShape::FlatQuad => xr_core::DisplayMode::FlatQuad,
            VirtualScreenShape::CurvedCylinder => xr_core::DisplayMode::CurvedCylinder,
        };

        DesktopLayerConfig {
            mode,
            quad_size: (self.screen_width_m, height_m),
            distance: self.distance_m,
            cylinder_radius: self.distance_m,
            cylinder_central_angle: self.cylinder_arc_rad,
            cylinder_aspect_ratio: display_aspect_ratio,
            pose: Some(Posef {
                orientation: Quaternionf::IDENTITY,
                position: Vector3f {
                    x: 0.0,
                    y: self.elevation_m,
                    z: -self.distance_m,
                },
            }),
        }
    }

    /// Builds an [`XrConfig`] wired for this screen's passthrough or dark-room environment.
    pub fn to_xr_config(
        &self,
        application_name: impl Into<std::borrow::Cow<'static, str>>,
        stream_width: u32,
        stream_height: u32,
    ) -> XrConfig {
        let mode = match self.shape {
            VirtualScreenShape::FlatQuad => xr_core::DisplayMode::FlatQuad,
            VirtualScreenShape::CurvedCylinder => xr_core::DisplayMode::CurvedCylinder,
        };
        let aspect = if stream_width > 0 && stream_height > 0 {
            stream_width as f32 / stream_height as f32
        } else {
            16.0 / 9.0
        };
        XrConfig {
            application_name: application_name.into(),
            stream_width,
            stream_height,
            display_mode: mode,
            quad_size: (self.screen_width_m, self.compute_screen_height_m(aspect)),
            quad_distance: self.distance_m,
            cylinder_radius: self.distance_m,
            cylinder_central_angle: self.cylinder_arc_rad,
            cylinder_aspect_ratio: aspect,
            enable_passthrough: self.environment.uses_passthrough(),
            ..Default::default()
        }
    }
}

/// Specifications for an individual eye's composition layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EyeLayerSpec {
    pub eye_visibility: EyeVisibility,
    pub sub_image_rect: StereoPixelRect,
}

/// Host-testable stereo virtual screen layout derived from a [`DetectedStereoFormat`].
#[derive(Debug, Clone, PartialEq)]
pub struct StereoScreenPlan {
    /// Composition layer visibility and pixel sub-image rectangle(s) (1 for mono, 2 for SBS/OU).
    pub eye_layers: Vec<EyeLayerSpec>,
    /// Per-eye normalized UV crop/scale and aspect correction `(left_eye, right_eye)`.
    pub eye_uv_crops: (EyeUvCrop, EyeUvCrop),
    /// Aspect-corrected physical display aspect ratio (`width / height`).
    pub display_aspect_ratio: f32,
    /// Physical flat-quad dimensions `(width_m, height_m)` on the virtual screen.
    pub quad_size_m: (f32, f32),
    /// Environment blend mode (`ALPHA_BLEND` for passthrough when supported, `OPAQUE` for dark room).
    pub blend_mode: EnvironmentBlendMode,
    /// Whether the passthrough reconstruction layer should be submitted behind the video layers.
    pub submit_passthrough_layer: bool,
}

/// Plans per-eye UV crop/scale, aspect-corrected virtual screen geometry, and passthrough/dark-room
/// backdrop from a [`DetectedStereoFormat`].
pub fn plan_stereo_virtual_screen(
    detected: DetectedStereoFormat,
    texture_width: u32,
    texture_height: u32,
    config: &VirtualScreenConfig,
    alpha_blend_supported: bool,
) -> Result<StereoScreenPlan> {
    let eye_layers = build_stereo_eye_layer_specs(
        detected.format,
        detected.eye_order,
        texture_width,
        texture_height,
    )?;
    let left_uv = calculate_eye_uv_crop(detected.format, detected.eye_order, Eye::Left);
    let right_uv = calculate_eye_uv_crop(detected.format, detected.eye_order, Eye::Right);
    let display_aspect_ratio =
        calculate_display_aspect_ratio(detected.format, texture_width, texture_height);
    let height_m = config.compute_screen_height_m(display_aspect_ratio);

    Ok(StereoScreenPlan {
        eye_layers,
        eye_uv_crops: (left_uv, right_uv),
        display_aspect_ratio,
        quad_size_m: (config.screen_width_m, height_m),
        blend_mode: config.environment.blend_mode(alpha_blend_supported),
        submit_passthrough_layer: config.environment.uses_passthrough(),
    })
}

/// Computes the composition layer specifications for left and right eyes based on stereo layout.
pub fn build_stereo_eye_layer_specs(
    format: StereoFormat,
    eye_order: EyeOrder,
    texture_width: u32,
    texture_height: u32,
) -> Result<Vec<EyeLayerSpec>> {
    if !is_packed_stereo(format) {
        // Mono (and any unpacked fallback) validates dimensions and uses a single layer visible to both eyes.
        let full_rect = calculate_eye_pixel_rect(
            StereoFormat::Mono,
            eye_order,
            Eye::Left,
            texture_width,
            texture_height,
        )?;
        return Ok(vec![EyeLayerSpec {
            eye_visibility: EyeVisibility::BOTH,
            sub_image_rect: full_rect,
        }]);
    }

    let left_rect =
        calculate_eye_pixel_rect(format, eye_order, Eye::Left, texture_width, texture_height)?;
    let right_rect =
        calculate_eye_pixel_rect(format, eye_order, Eye::Right, texture_width, texture_height)?;

    Ok(vec![
        EyeLayerSpec {
            eye_visibility: EyeVisibility::LEFT,
            sub_image_rect: left_rect,
        },
        EyeLayerSpec {
            eye_visibility: EyeVisibility::RIGHT,
            sub_image_rect: right_rect,
        },
    ])
}

/// Constructs OpenXR Quad composition layers for stereo virtual screen presentation.
pub fn build_stereo_quad_layers<'a, G: Graphics>(
    space: &'a Space,
    swapchain: &'a Swapchain<G>,
    format: StereoFormat,
    eye_order: EyeOrder,
    texture_width: u32,
    texture_height: u32,
    config: &VirtualScreenConfig,
) -> Result<Vec<CompositionLayerQuad<'a, G>>> {
    let pose = Posef {
        orientation: Quaternionf::IDENTITY,
        position: Vector3f {
            x: 0.0,
            y: config.elevation_m,
            z: -config.distance_m,
        },
    };

    build_stereo_quad_layers_at_pose(
        space,
        swapchain,
        format,
        eye_order,
        texture_width,
        texture_height,
        config,
        pose,
    )
}

/// Builds eye layers at an explicitly anchored cinema pose in the supplied reference space.
#[allow(clippy::too_many_arguments)]
pub fn build_stereo_quad_layers_at_pose<'a, G: Graphics>(
    space: &'a Space,
    swapchain: &'a Swapchain<G>,
    format: StereoFormat,
    eye_order: EyeOrder,
    texture_width: u32,
    texture_height: u32,
    config: &VirtualScreenConfig,
    pose: Posef,
) -> Result<Vec<CompositionLayerQuad<'a, G>>> {
    let specs = build_stereo_eye_layer_specs(format, eye_order, texture_width, texture_height)?;
    let aspect = calculate_display_aspect_ratio(format, texture_width, texture_height);
    let height_m = config.compute_screen_height_m(aspect);

    let size = Extent2Df {
        width: config.screen_width_m,
        height: height_m,
    };

    let layers = specs
        .into_iter()
        .map(|spec| {
            let sub_image = SwapchainSubImage::new()
                .swapchain(swapchain)
                .image_rect(Rect2Di {
                    offset: Offset2Di {
                        x: spec.sub_image_rect.x as i32,
                        y: spec.sub_image_rect.y as i32,
                    },
                    extent: Extent2Di {
                        width: spec.sub_image_rect.width as i32,
                        height: spec.sub_image_rect.height as i32,
                    },
                });

            CompositionLayerQuad::new()
                .layer_flags(CompositionLayerFlags::CORRECT_CHROMATIC_ABERRATION)
                .space(space)
                .eye_visibility(spec.eye_visibility)
                .sub_image(sub_image)
                .pose(pose)
                .size(size)
        })
        .collect();

    Ok(layers)
}

/// Constructs OpenXR Curved Cylinder composition layers for immersive cinema curved presentation.
pub fn build_stereo_cylinder_layers<'a, G: Graphics>(
    space: &'a Space,
    swapchain: &'a Swapchain<G>,
    format: StereoFormat,
    eye_order: EyeOrder,
    texture_width: u32,
    texture_height: u32,
    config: &VirtualScreenConfig,
) -> Result<Vec<CompositionLayerCylinderKHR<'a, G>>> {
    let aspect = calculate_display_aspect_ratio(format, texture_width, texture_height);

    let pose = Posef {
        orientation: Quaternionf::IDENTITY,
        position: Vector3f {
            x: 0.0,
            y: config.elevation_m,
            z: 0.0,
        },
    };

    build_stereo_cylinder_layers_at_pose(
        space,
        swapchain,
        format,
        eye_order,
        texture_width,
        texture_height,
        pose,
        config.distance_m,
        config.cylinder_arc_rad,
        aspect,
    )
}

/// Explicit cylinder axis pose, matching DeskQuest's curved-surface hit testing.
#[allow(clippy::too_many_arguments)]
pub fn build_stereo_cylinder_layers_at_pose<'a, G: Graphics>(
    space: &'a Space,
    swapchain: &'a Swapchain<G>,
    format: StereoFormat,
    eye_order: EyeOrder,
    texture_width: u32,
    texture_height: u32,
    pose: Posef,
    radius: f32,
    angle: f32,
    aspect: f32,
) -> Result<Vec<CompositionLayerCylinderKHR<'a, G>>> {
    let specs = build_stereo_eye_layer_specs(format, eye_order, texture_width, texture_height)?;

    let layers = specs
        .into_iter()
        .map(|spec| {
            let sub_image = SwapchainSubImage::new()
                .swapchain(swapchain)
                .image_rect(Rect2Di {
                    offset: Offset2Di {
                        x: spec.sub_image_rect.x as i32,
                        y: spec.sub_image_rect.y as i32,
                    },
                    extent: Extent2Di {
                        width: spec.sub_image_rect.width as i32,
                        height: spec.sub_image_rect.height as i32,
                    },
                });

            CompositionLayerCylinderKHR::new()
                .layer_flags(CompositionLayerFlags::CORRECT_CHROMATIC_ABERRATION)
                .space(space)
                .eye_visibility(spec.eye_visibility)
                .sub_image(sub_image)
                .pose(pose)
                .radius(radius)
                .central_angle(angle)
                .aspect_ratio(aspect)
        })
        .collect();

    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_screen_height_computation() {
        let config = VirtualScreenConfig {
            screen_width_m: 2.6,
            ..Default::default()
        };
        // 16:9 aspect ratio ≈ 1.7777
        let height = config.compute_screen_height_m(16.0 / 9.0);
        let expected = 2.6 / (16.0 / 9.0);
        assert!((height - expected).abs() < 1e-4);
    }

    #[test]
    fn test_eye_layer_specs_for_sbs() {
        let specs = build_stereo_eye_layer_specs(
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            1920,
            1080,
        )
        .unwrap();

        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].eye_visibility, EyeVisibility::LEFT);
        assert_eq!(
            specs[0].sub_image_rect,
            StereoPixelRect {
                x: 0,
                y: 0,
                width: 960,
                height: 1080
            }
        );

        assert_eq!(specs[1].eye_visibility, EyeVisibility::RIGHT);
        assert_eq!(
            specs[1].sub_image_rect,
            StereoPixelRect {
                x: 960,
                y: 0,
                width: 960,
                height: 1080
            }
        );
    }

    #[test]
    fn test_eye_layer_specs_for_mono() {
        let specs =
            build_stereo_eye_layer_specs(StereoFormat::Mono, EyeOrder::LeftRight, 1920, 1080)
                .unwrap();

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].eye_visibility, EyeVisibility::BOTH);
        assert_eq!(
            specs[0].sub_image_rect,
            StereoPixelRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
    }
}
