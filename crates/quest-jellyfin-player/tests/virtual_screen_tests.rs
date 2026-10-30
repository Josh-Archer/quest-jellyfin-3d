use openxr::EyeVisibility;
use quest_jellyfin_player::{
    build_stereo_eye_layer_specs, StereoPixelRect, VirtualScreenConfig, VirtualScreenEnvironment,
    VirtualScreenShape,
};
use quest_jellyfin_player::{EyeOrder, StereoFormat};

#[test]
fn test_virtual_screen_config_and_desktop_layer() {
    let mut config = VirtualScreenConfig::default();
    assert_eq!(config.environment, VirtualScreenEnvironment::Passthrough);
    assert_eq!(config.shape, VirtualScreenShape::CurvedCylinder);
    assert_eq!(config.screen_width_m, 4.0);
    assert_eq!(config.distance_m, 2.2);

    // Switch to dark cinema room and flat quad
    config.environment = VirtualScreenEnvironment::DarkRoom;
    config.shape = VirtualScreenShape::FlatQuad;

    let desktop_layer = config.to_desktop_layer_config(16.0 / 9.0);
    assert_eq!(desktop_layer.mode, xr_core::DisplayMode::FlatQuad);
    assert_eq!(desktop_layer.quad_size.0, 4.0);
    let expected_height = 4.0 / (16.0 / 9.0);
    assert!((desktop_layer.quad_size.1 - expected_height).abs() < 1e-4);
    assert_eq!(desktop_layer.distance, 2.2);
}

#[test]
fn test_stereo_layer_specs_generation() {
    // 1. SBS standard LeftRight
    let sbs_specs = build_stereo_eye_layer_specs(
        StereoFormat::HalfSideBySide,
        EyeOrder::LeftRight,
        3840,
        1080,
    )
    .unwrap();
    assert_eq!(sbs_specs.len(), 2);
    assert_eq!(sbs_specs[0].eye_visibility, EyeVisibility::LEFT);
    assert_eq!(
        sbs_specs[0].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(sbs_specs[1].eye_visibility, EyeVisibility::RIGHT);
    assert_eq!(
        sbs_specs[1].sub_image_rect,
        StereoPixelRect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080
        }
    );

    // 2. SBS swapped RightLeft
    let sbs_swapped = build_stereo_eye_layer_specs(
        StereoFormat::FullSideBySide,
        EyeOrder::RightLeft,
        3840,
        1080,
    )
    .unwrap();
    assert_eq!(sbs_swapped.len(), 2);
    // Left eye now receives the right region (x = 1920)
    assert_eq!(sbs_swapped[0].eye_visibility, EyeVisibility::LEFT);
    assert_eq!(
        sbs_swapped[0].sub_image_rect,
        StereoPixelRect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    // Right eye now receives the left region (x = 0)
    assert_eq!(sbs_swapped[1].eye_visibility, EyeVisibility::RIGHT);
    assert_eq!(
        sbs_swapped[1].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );

    // 3. OU standard TopBottom
    let ou_specs =
        build_stereo_eye_layer_specs(StereoFormat::FullOverUnder, EyeOrder::TopBottom, 1920, 2160)
            .unwrap();
    assert_eq!(ou_specs.len(), 2);
    assert_eq!(ou_specs[0].eye_visibility, EyeVisibility::LEFT);
    assert_eq!(
        ou_specs[0].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(ou_specs[1].eye_visibility, EyeVisibility::RIGHT);
    assert_eq!(
        ou_specs[1].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 1080,
            width: 1920,
            height: 1080
        }
    );

    // 4. OU swapped BottomTop
    let ou_swapped =
        build_stereo_eye_layer_specs(StereoFormat::HalfOverUnder, EyeOrder::BottomTop, 1920, 1080)
            .unwrap();
    assert_eq!(ou_swapped.len(), 2);
    // Left eye receives bottom half
    assert_eq!(ou_swapped[0].eye_visibility, EyeVisibility::LEFT);
    assert_eq!(
        ou_swapped[0].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 540,
            width: 1920,
            height: 540
        }
    );
    // Right eye receives top half
    assert_eq!(ou_swapped[1].eye_visibility, EyeVisibility::RIGHT);
    assert_eq!(
        ou_swapped[1].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 540
        }
    );

    // 5. Mono
    let mono_specs =
        build_stereo_eye_layer_specs(StereoFormat::Mono, EyeOrder::LeftRight, 1920, 1080).unwrap();
    assert_eq!(mono_specs.len(), 1);
    assert_eq!(mono_specs[0].eye_visibility, EyeVisibility::BOTH);
    assert_eq!(
        mono_specs[0].sub_image_rect,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
}
