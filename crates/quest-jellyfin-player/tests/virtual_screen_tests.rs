use jellyfin_client::{BaseItem, Video3DFormat};
use openxr::{EnvironmentBlendMode, EyeVisibility};
use quest_jellyfin_player::{
    build_stereo_eye_layer_specs, detect_item_format, plan_stereo_virtual_screen, StereoPixelRect,
    VirtualScreenConfig, VirtualScreenEnvironment, VirtualScreenShape,
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

    // 5. Mono falls back to a single layer visible to both eyes, while rejecting invalid bounds
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
    assert!(
        build_stereo_eye_layer_specs(StereoFormat::Mono, EyeOrder::LeftRight, 0, 1080).is_err()
    );
    assert!(
        build_stereo_eye_layer_specs(StereoFormat::Mono, EyeOrder::LeftRight, u32::MAX, 1080)
            .is_err()
    );
}

#[test]
fn test_detected_format_plans_stereo_screen_and_backdrop_modes() {
    let eps = 1e-4;
    let passthrough_cfg = VirtualScreenConfig {
        environment: VirtualScreenEnvironment::Passthrough,
        shape: VirtualScreenShape::CurvedCylinder,
        screen_width_m: 4.0,
        ..Default::default()
    };
    let dark_room_cfg = VirtualScreenConfig {
        environment: VirtualScreenEnvironment::DarkRoom,
        shape: VirtualScreenShape::FlatQuad,
        screen_width_m: 4.0,
        ..Default::default()
    };

    assert!(
        passthrough_cfg
            .to_xr_config("Jellyfin 3D", 1920, 1080)
            .enable_passthrough
    );
    assert!(
        !dark_room_cfg
            .to_xr_config("Jellyfin 3D", 1920, 1080)
            .enable_passthrough
    );

    // Half SBS item from Jellyfin metadata -> 2 eyes, 2x horizontal aspect correction -> 16:9 screen
    let hsbs_item = BaseItem {
        video_3d_format: Some(Video3DFormat::HalfSideBySide),
        path: Some("/media/Avatar.HSBS.1080p.mkv".into()),
        ..Default::default()
    };
    let hsbs = plan_stereo_virtual_screen(
        detect_item_format(&hsbs_item),
        1920,
        1080,
        &passthrough_cfg,
        true,
    )
    .unwrap();
    assert_eq!(hsbs.eye_layers.len(), 2);
    assert_eq!(hsbs.eye_uv_crops.0.offset, (0.0, 0.0));
    assert_eq!(hsbs.eye_uv_crops.0.scale, (0.5, 1.0));
    assert_eq!(hsbs.eye_uv_crops.0.aspect_scale, (2.0, 1.0));
    assert_eq!(hsbs.eye_uv_crops.1.offset, (0.5, 0.0));
    assert!((hsbs.quad_size_m.1 - 2.25).abs() < eps);
    assert_eq!(hsbs.blend_mode, EnvironmentBlendMode::ALPHA_BLEND);
    assert!(hsbs.submit_passthrough_layer);

    // Passthrough on a runtime without alpha-blend falls back to OPAQUE blend mode
    let hsbs_no_alpha = plan_stereo_virtual_screen(
        detect_item_format(&hsbs_item),
        1920,
        1080,
        &passthrough_cfg,
        false,
    )
    .unwrap();
    assert_eq!(hsbs_no_alpha.blend_mode, EnvironmentBlendMode::OPAQUE);

    // Half OU item with reversed BT filename tag in dark room -> 2x vertical aspect correction, OPAQUE backdrop
    let hou_item = BaseItem {
        video_3d_format: Some(Video3DFormat::HalfTopAndBottom),
        path: Some("/media/Coraline_BT.mkv".into()),
        ..Default::default()
    };
    let hou = plan_stereo_virtual_screen(
        detect_item_format(&hou_item),
        1920,
        1080,
        &dark_room_cfg,
        true,
    )
    .unwrap();
    assert_eq!(hou.eye_layers.len(), 2);
    assert_eq!(hou.eye_uv_crops.0.offset, (0.0, 0.5));
    assert_eq!(hou.eye_uv_crops.1.offset, (0.0, 0.0));
    assert_eq!(hou.eye_uv_crops.0.aspect_scale, (1.0, 2.0));
    assert!((hou.quad_size_m.1 - 2.25).abs() < eps);
    assert_eq!(hou.blend_mode, EnvironmentBlendMode::OPAQUE);
    assert!(!hou.submit_passthrough_layer);

    // Plain 2D movie -> mono fallback to BOTH eyes in dark room
    let mono_item = BaseItem {
        path: Some("/media/Arrival (2016) 1080p.mkv".into()),
        ..Default::default()
    };
    let mono = plan_stereo_virtual_screen(
        detect_item_format(&mono_item),
        1920,
        1080,
        &dark_room_cfg,
        true,
    )
    .unwrap();
    assert_eq!(mono.eye_layers.len(), 1);
    assert_eq!(mono.eye_layers[0].eye_visibility, EyeVisibility::BOTH);
    assert_eq!(mono.eye_uv_crops.0.scale, (1.0, 1.0));
    assert_eq!(mono.eye_uv_crops.1.scale, (1.0, 1.0));
}
