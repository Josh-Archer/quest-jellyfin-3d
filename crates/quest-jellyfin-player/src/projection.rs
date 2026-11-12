//! Spherical composition layers without copying or downsampling the decoded eye regions.
//!
//! Equirectangular 180/360 use `CompositionLayerEquirect2KHR` with longitude/latitude
//! sampling. Fisheye takes a separate equidistant UV-mesh remap path
//! ([`build_fisheye_remap_layers`]); OpenXR still has no fisheye layer, so that path
//! submits hemispherical Equirect2 carriers while the mesh encodes polar lens UVs
//! (see `fisheye` and docs/player-design.md).
use crate::fisheye::{
    spherical_submission, EquidistantFisheye, FisheyeRemapMesh, SphericalSubmission,
};
use crate::{build_stereo_eye_layer_specs, EyeOrder, Projection, Result, StereoFormat};
use openxr::{CompositionLayerEquirect2KHR, Posef, Space, Swapchain, Vulkan};

pub use crate::fisheye::{spherical_submission as resolve_spherical_submission, FisheyeMeshVertex};

/// Horizontal coverage of the OpenXR sphere carrier, when the projection is spherical.
///
/// Fisheye uses a hemispherical carrier (π) for the equidistant remap path; that is
/// not the same as treating the source as VR180 equirectangular.
pub fn horizontal_angle(projection: Projection) -> Option<f32> {
    match projection {
        Projection::Equirectangular180 | Projection::Fisheye => Some(std::f32::consts::PI),
        Projection::Equirectangular360 => Some(std::f32::consts::TAU),
        Projection::Flat => None,
    }
}

/// Per-title horizontal correction, in half-degree steps of relative eye angle.
/// This changes view orientation, never source crops, resolution or headset IPD.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StereoAlignment {
    steps: i16,
}
impl StereoAlignment {
    pub fn from_degrees(value: f32) -> Self {
        Self {
            steps: if value.is_finite() {
                (value.clamp(-8.0, 8.0) * 2.0).round() as i16
            } else {
                0
            },
        }
    }
    pub fn degrees(self) -> f32 {
        f32::from(self.steps) * 0.5
    }
    pub fn adjust_steps(&mut self, steps: i16) {
        self.steps = self.steps.saturating_add(steps).clamp(-16, 16);
    }
    pub fn eye_pose(self, pose: Posef, eye: openxr::EyeVisibility) -> Posef {
        // A positive correction adds to right-minus-left displayed longitude.
        // Positive OpenXR yaw rotates a forward ray toward negative longitude.
        let sign = if eye == openxr::EyeVisibility::LEFT {
            1.0
        } else if eye == openxr::EyeVisibility::RIGHT {
            -1.0
        } else {
            return pose;
        };
        if self.steps == 0 {
            return pose;
        }
        let (s, c) = (sign * self.degrees().to_radians() / 4.0).sin_cos();
        let q = pose.orientation;
        Posef {
            orientation: openxr::Quaternionf {
                x: q.x * c - q.z * s,
                y: q.y * c + q.w * s,
                z: q.z * c + q.x * s,
                w: q.w * c - q.y * s,
            },
            ..pose
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_equirect_layers<'a>(
    space: &'a Space,
    swapchain: &'a Swapchain<Vulkan>,
    format: StereoFormat,
    order: EyeOrder,
    width: u32,
    height: u32,
    projection: Projection,
    pose: Posef,
    comfort_mono: bool,
) -> Result<Vec<CompositionLayerEquirect2KHR<'a, Vulkan>>> {
    build_equirect_layers_aligned(
        space,
        swapchain,
        format,
        order,
        width,
        height,
        projection,
        pose,
        comfort_mono,
        StereoAlignment::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_equirect_layers_aligned<'a>(
    space: &'a Space,
    swapchain: &'a Swapchain<Vulkan>,
    format: StereoFormat,
    order: EyeOrder,
    width: u32,
    height: u32,
    projection: Projection,
    pose: Posef,
    comfort_mono: bool,
    alignment: StereoAlignment,
) -> Result<Vec<CompositionLayerEquirect2KHR<'a, Vulkan>>> {
    match spherical_submission(projection) {
        Some(SphericalSubmission::Equirect180 | SphericalSubmission::Equirect360) => {}
        Some(SphericalSubmission::FisheyeEquidistantRemap) => {
            return Err(crate::PlayerError::Playback(
                "Fisheye requires build_fisheye_remap_layers, not plain equirect".into(),
            ));
        }
        None => {
            return Err(crate::PlayerError::Playback(
                "Expected an equirectangular projection".into(),
            ));
        }
    }
    let angle = horizontal_angle(projection)
        .ok_or_else(|| crate::PlayerError::Playback("Expected a spherical projection".into()))?;
    let specs = equirect_eye_specs(format, order, width, height, comfort_mono)?;
    Ok(specs
        .into_iter()
        .map(|spec| {
            CompositionLayerEquirect2KHR::new()
                .layer_flags(openxr::CompositionLayerFlags::CORRECT_CHROMATIC_ABERRATION)
                .space(space)
                .eye_visibility(spec.eye_visibility)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(swapchain)
                        .image_rect(openxr::Rect2Di {
                            offset: openxr::Offset2Di {
                                x: spec.sub_image_rect.x as i32,
                                y: spec.sub_image_rect.y as i32,
                            },
                            extent: openxr::Extent2Di {
                                width: spec.sub_image_rect.width as i32,
                                height: spec.sub_image_rect.height as i32,
                            },
                        }),
                )
                .pose(alignment.eye_pose(pose, spec.eye_visibility))
                .radius(0.0)
                .central_horizontal_angle(angle)
                .upper_vertical_angle(std::f32::consts::FRAC_PI_2)
                .lower_vertical_angle(-std::f32::consts::FRAC_PI_2)
        })
        .collect())
}

/// Equidistant fisheye remap: polar UV mesh plus hemispherical Equirect2 carriers.
///
/// Distinct from [`build_equirect_layers`]: classified Fisheye never shares the plain
/// VR180 equirect builder. OpenXR has no fisheye layer, so the mesh carries the lens
/// UVs while Equirect2 places the sphere.
#[allow(clippy::too_many_arguments)]
pub fn build_fisheye_remap_layers<'a>(
    space: &'a Space,
    swapchain: &'a Swapchain<Vulkan>,
    format: StereoFormat,
    order: EyeOrder,
    width: u32,
    height: u32,
    pose: Posef,
    comfort_mono: bool,
    alignment: StereoAlignment,
) -> Result<(
    FisheyeRemapMesh,
    Vec<CompositionLayerEquirect2KHR<'a, Vulkan>>,
)> {
    build_fisheye_remap_layers_with_lens(
        space,
        swapchain,
        format,
        order,
        width,
        height,
        pose,
        comfort_mono,
        alignment,
        EquidistantFisheye::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_fisheye_remap_layers_with_lens<'a>(
    space: &'a Space,
    swapchain: &'a Swapchain<Vulkan>,
    format: StereoFormat,
    order: EyeOrder,
    width: u32,
    height: u32,
    pose: Posef,
    comfort_mono: bool,
    alignment: StereoAlignment,
    lens: EquidistantFisheye,
) -> Result<(
    FisheyeRemapMesh,
    Vec<CompositionLayerEquirect2KHR<'a, Vulkan>>,
)> {
    let mesh = FisheyeRemapMesh::hemisphere(lens, 16, 32);
    let angle = horizontal_angle(Projection::Fisheye)
        .ok_or_else(|| crate::PlayerError::Playback("Expected fisheye projection".into()))?;
    let specs = equirect_eye_specs(format, order, width, height, comfort_mono)?;
    let layers = specs
        .into_iter()
        .map(|spec| {
            CompositionLayerEquirect2KHR::new()
                .layer_flags(openxr::CompositionLayerFlags::CORRECT_CHROMATIC_ABERRATION)
                .space(space)
                .eye_visibility(spec.eye_visibility)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(swapchain)
                        .image_rect(openxr::Rect2Di {
                            offset: openxr::Offset2Di {
                                x: spec.sub_image_rect.x as i32,
                                y: spec.sub_image_rect.y as i32,
                            },
                            extent: openxr::Extent2Di {
                                width: spec.sub_image_rect.width as i32,
                                height: spec.sub_image_rect.height as i32,
                            },
                        }),
                )
                .pose(alignment.eye_pose(pose, spec.eye_visibility))
                .radius(0.0)
                .central_horizontal_angle(angle)
                .upper_vertical_angle(std::f32::consts::FRAC_PI_2)
                .lower_vertical_angle(-std::f32::consts::FRAC_PI_2)
        })
        .collect();
    Ok((mesh, layers))
}

/// A 2D comfort view keeps a complete source eye and makes that same crop binocular.
/// It must not display the full packed frame or two overlapping BOTH-eye layers.
pub fn equirect_eye_specs(
    format: StereoFormat,
    order: EyeOrder,
    width: u32,
    height: u32,
    mono: bool,
) -> Result<Vec<crate::EyeLayerSpec>> {
    let mut specs = build_stereo_eye_layer_specs(format, order, width, height)?;
    if mono {
        specs.truncate(1);
        specs[0].eye_visibility = openxr::EyeVisibility::BOTH;
    }
    Ok(specs)
}

/// Manual choices override detection; unspecified spherical coverage is inferred only when
/// explicit VR/equirectangular metadata exists. Fisheye is never inferred from aspect alone.
pub fn resolve_projection(
    choice: &str,
    detected: Projection,
    tags: &[String],
    per_eye_aspect: f32,
) -> Projection {
    match choice {
        "Flat" => return Projection::Flat,
        "Equirectangular180" => return Projection::Equirectangular180,
        "Equirectangular360" => return Projection::Equirectangular360,
        "Fisheye" => return Projection::Fisheye,
        _ => {}
    }
    for tag in tags {
        match tag.to_ascii_lowercase().as_str() {
            "fisheye" => return Projection::Fisheye,
            "vr180" => return Projection::Equirectangular180,
            "vr360" => return Projection::Equirectangular360,
            _ => {}
        }
    }
    if detected != Projection::Flat {
        return detected;
    }
    if tags
        .iter()
        .any(|t| t.eq_ignore_ascii_case("equirectangular"))
    {
        return if per_eye_aspect > 1.5 {
            Projection::Equirectangular360
        } else {
            Projection::Equirectangular180
        };
    }
    Projection::Flat
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_is_bounded_reversible_and_invalid_values_are_neutral() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                StereoAlignment::from_degrees(value),
                StereoAlignment::default()
            );
        }
        let mut alignment = StereoAlignment::default();
        alignment.adjust_steps(1);
        assert_eq!(alignment.degrees(), 0.5);
        alignment.adjust_steps(-1);
        assert_eq!(alignment.degrees(), 0.0);
        alignment.adjust_steps(i16::MAX);
        assert_eq!(alignment.degrees(), 8.0);
        alignment.adjust_steps(i16::MIN);
        assert_eq!(alignment.degrees(), -8.0);
    }
    #[test]
    fn relative_correction_aligns_a_known_eight_degree_source_disparity() {
        let pose = Posef::IDENTITY;
        let alignment = StereoAlignment::from_degrees(8.0);
        let left = alignment.eye_pose(pose, openxr::EyeVisibility::LEFT);
        let right = alignment.eye_pose(pose, openxr::EyeVisibility::RIGHT);
        let yaw = |p: Posef| 2.0 * p.orientation.y.atan2(p.orientation.w);
        let displayed_left = 4_f32.to_radians() - yaw(left);
        let displayed_right = -4_f32.to_radians() - yaw(right);
        assert!((displayed_left - displayed_right).abs() < 1e-6);
        assert_eq!(left.position, pose.position);
        assert_eq!(right.position, pose.position);
        assert_eq!(alignment.eye_pose(pose, openxr::EyeVisibility::BOTH), pose);
        assert_eq!(
            StereoAlignment::default().eye_pose(pose, openxr::EyeVisibility::LEFT),
            pose
        );
        assert_eq!(
            StereoAlignment::default().eye_pose(pose, openxr::EyeVisibility::RIGHT),
            pose
        );
    }
    #[test]
    fn spherical_coverage_and_explicit_overrides_do_not_change_packing() {
        assert_eq!(
            horizontal_angle(Projection::Equirectangular180),
            Some(std::f32::consts::PI)
        );
        assert_eq!(
            horizontal_angle(Projection::Equirectangular360),
            Some(std::f32::consts::TAU)
        );
        assert_eq!(
            horizontal_angle(Projection::Fisheye),
            Some(std::f32::consts::PI)
        );
        assert_eq!(
            resolve_spherical_submission(Projection::Fisheye),
            Some(SphericalSubmission::FisheyeEquidistantRemap)
        );
        assert_ne!(
            resolve_spherical_submission(Projection::Fisheye),
            resolve_spherical_submission(Projection::Equirectangular180)
        );
        assert_eq!(horizontal_angle(Projection::Flat), None);
        let tags = vec!["Equirectangular".into()];
        assert_eq!(
            resolve_projection("Auto", Projection::Flat, &tags, 1.0),
            Projection::Equirectangular180
        );
        assert_eq!(
            resolve_projection("Auto", Projection::Flat, &tags, 2.0),
            Projection::Equirectangular360
        );
        assert_eq!(
            resolve_projection("Flat", Projection::Equirectangular180, &tags, 1.0),
            Projection::Flat
        );
        assert_eq!(
            resolve_projection("Auto", Projection::Flat, &[], 1.0),
            Projection::Flat
        );
        assert_eq!(
            resolve_projection("Equirectangular360", Projection::Flat, &tags, 1.0),
            Projection::Equirectangular360
        );
        assert_eq!(
            resolve_projection("Fisheye", Projection::Flat, &[], 2.0),
            Projection::Fisheye
        );
        assert_eq!(
            resolve_projection("Auto", Projection::Fisheye, &[], 2.0),
            Projection::Fisheye
        );
        assert_eq!(
            resolve_projection("Auto", Projection::Flat, &["Fisheye".into()], 2.0),
            Projection::Fisheye
        );
        // Aspect alone must not invent fisheye for ordinary flat movies.
        assert_eq!(
            resolve_projection("Auto", Projection::Flat, &[], 1.0),
            Projection::Flat
        );
    }
    #[test]
    fn comfort_view_uses_one_complete_eye_for_both_eyes_in_either_order() {
        for order in [EyeOrder::LeftRight, EyeOrder::RightLeft] {
            let stereo =
                equirect_eye_specs(StereoFormat::FullSideBySide, order, 7680, 3840, false).unwrap();
            let mono =
                equirect_eye_specs(StereoFormat::FullSideBySide, order, 7680, 3840, true).unwrap();
            assert_eq!(stereo.len(), 2);
            assert_eq!(mono.len(), 1);
            assert_eq!(mono[0].eye_visibility, openxr::EyeVisibility::BOTH);
            assert_eq!(mono[0].sub_image_rect, stereo[0].sub_image_rect);
            assert_eq!(mono[0].sub_image_rect.width, 3840);
            assert_eq!(
                mono[0].sub_image_rect.x,
                if order == EyeOrder::LeftRight {
                    0
                } else {
                    3840
                }
            );
        }
    }
    #[test]
    fn full_resolution_stereo_regions_remain_separate_for_vr() {
        let specs = build_stereo_eye_layer_specs(
            StereoFormat::FullSideBySide,
            EyeOrder::RightLeft,
            7680,
            3840,
        )
        .unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].sub_image_rect.width, 3840);
        assert_eq!(specs[0].sub_image_rect.height, 3840);
        assert_ne!(specs[0].sub_image_rect.x, specs[1].sub_image_rect.x);
    }
}
