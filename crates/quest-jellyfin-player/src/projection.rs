//! Equirectangular projection without copying or downsampling the decoded eye regions.
use crate::{build_stereo_eye_layer_specs, EyeOrder, Projection, Result, StereoFormat};
use openxr::{CompositionLayerEquirect2KHR, Posef, Space, Swapchain, Vulkan};

pub fn horizontal_angle(projection: Projection) -> Option<f32> {
    match projection {
        Projection::Equirectangular180 => Some(std::f32::consts::PI),
        Projection::Equirectangular360 => Some(std::f32::consts::TAU),
        _ => None,
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
) -> Result<Vec<CompositionLayerEquirect2KHR<'a, Vulkan>>> {
    let angle = horizontal_angle(projection).ok_or_else(|| {
        crate::PlayerError::Playback("Expected equirectangular projection".into())
    })?;
    let specs = build_stereo_eye_layer_specs(format, order, width, height)?;
    Ok(specs
        .into_iter()
        .map(|spec| {
            CompositionLayerEquirect2KHR::new()
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
                .pose(pose)
                .radius(0.0)
                .central_horizontal_angle(angle)
                .upper_vertical_angle(std::f32::consts::FRAC_PI_2)
                .lower_vertical_angle(-std::f32::consts::FRAC_PI_2)
        })
        .collect())
}

/// Manual choices override detection; unspecified spherical coverage is inferred only when
/// explicit VR/equirectangular metadata exists. The UI always offers 180/360 correction.
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
        _ => {}
    }
    for tag in tags {
        match tag.to_ascii_lowercase().as_str() {
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
    fn spherical_coverage_and_explicit_overrides_do_not_change_packing() {
        assert_eq!(
            horizontal_angle(Projection::Equirectangular180),
            Some(std::f32::consts::PI)
        );
        assert_eq!(
            horizontal_angle(Projection::Equirectangular360),
            Some(std::f32::consts::TAU)
        );
        assert_eq!(horizontal_angle(Projection::Fisheye), None);
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
