use quest_jellyfin_player::{
    calculate_display_aspect_ratio, calculate_eye_pixel_rect, calculate_eye_uv, Eye,
    NormalizedUvRect, StereoPixelRect,
};
use quest_jellyfin_player::{EyeOrder, StereoFormat};

#[test]
fn test_uv_math_all_formats_and_permutations() {
    // 1. Mono
    assert_eq!(
        calculate_eye_uv(StereoFormat::Mono, EyeOrder::LeftRight, Eye::Left),
        NormalizedUvRect::FULL_FRAME
    );
    assert_eq!(
        calculate_eye_uv(StereoFormat::Mono, EyeOrder::LeftRight, Eye::Right),
        NormalizedUvRect::FULL_FRAME
    );

    // 2. Half SBS (LeftRight standard)
    let hsbs_l = calculate_eye_uv(StereoFormat::HalfSideBySide, EyeOrder::LeftRight, Eye::Left);
    let hsbs_r = calculate_eye_uv(
        StereoFormat::HalfSideBySide,
        EyeOrder::LeftRight,
        Eye::Right,
    );
    assert_eq!(
        hsbs_l,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 0.5,
            v_max: 1.0
        }
    );
    assert_eq!(
        hsbs_r,
        NormalizedUvRect {
            u_min: 0.5,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 1.0
        }
    );

    // 3. Half SBS (RightLeft swapped)
    let hsbs_swapped_l =
        calculate_eye_uv(StereoFormat::HalfSideBySide, EyeOrder::RightLeft, Eye::Left);
    let hsbs_swapped_r = calculate_eye_uv(
        StereoFormat::HalfSideBySide,
        EyeOrder::RightLeft,
        Eye::Right,
    );
    assert_eq!(
        hsbs_swapped_l,
        NormalizedUvRect {
            u_min: 0.5,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 1.0
        }
    );
    assert_eq!(
        hsbs_swapped_r,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 0.5,
            v_max: 1.0
        }
    );

    // 4. Full SBS (LeftRight standard)
    let fsbs_l = calculate_eye_uv(StereoFormat::FullSideBySide, EyeOrder::LeftRight, Eye::Left);
    let fsbs_r = calculate_eye_uv(
        StereoFormat::FullSideBySide,
        EyeOrder::LeftRight,
        Eye::Right,
    );
    assert_eq!(
        fsbs_l,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 0.5,
            v_max: 1.0
        }
    );
    assert_eq!(
        fsbs_r,
        NormalizedUvRect {
            u_min: 0.5,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 1.0
        }
    );

    // 5. Half OverUnder (TopBottom standard)
    let hou_l = calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::TopBottom, Eye::Left);
    let hou_r = calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::TopBottom, Eye::Right);
    assert_eq!(
        hou_l,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 0.5
        }
    );
    assert_eq!(
        hou_r,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.5,
            u_max: 1.0,
            v_max: 1.0
        }
    );

    // 6. Half OverUnder (BottomTop swapped)
    let hou_swapped_l =
        calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::BottomTop, Eye::Left);
    let hou_swapped_r =
        calculate_eye_uv(StereoFormat::HalfOverUnder, EyeOrder::BottomTop, Eye::Right);
    assert_eq!(
        hou_swapped_l,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.5,
            u_max: 1.0,
            v_max: 1.0
        }
    );
    assert_eq!(
        hou_swapped_r,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 0.5
        }
    );

    // 7. Full OverUnder (TopBottom standard)
    let fou_l = calculate_eye_uv(StereoFormat::FullOverUnder, EyeOrder::TopBottom, Eye::Left);
    let fou_r = calculate_eye_uv(StereoFormat::FullOverUnder, EyeOrder::TopBottom, Eye::Right);
    assert_eq!(
        fou_l,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 0.5
        }
    );
    assert_eq!(
        fou_r,
        NormalizedUvRect {
            u_min: 0.0,
            v_min: 0.5,
            u_max: 1.0,
            v_max: 1.0
        }
    );
}

#[test]
fn test_pixel_partitioning_resolutions() {
    // 1080p Half SBS: 1920x1080 -> 960x1080 each
    let hsbs_l = calculate_eye_pixel_rect(
        StereoFormat::HalfSideBySide,
        EyeOrder::LeftRight,
        Eye::Left,
        1920,
        1080,
    )
    .unwrap();
    let hsbs_r = calculate_eye_pixel_rect(
        StereoFormat::HalfSideBySide,
        EyeOrder::LeftRight,
        Eye::Right,
        1920,
        1080,
    )
    .unwrap();
    assert_eq!(
        hsbs_l,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 960,
            height: 1080
        }
    );
    assert_eq!(
        hsbs_r,
        StereoPixelRect {
            x: 960,
            y: 0,
            width: 960,
            height: 1080
        }
    );
    assert_eq!(hsbs_l.width + hsbs_r.width, 1920);

    // 4K Full SBS: 3840x1080 -> 1920x1080 each
    let fsbs_l = calculate_eye_pixel_rect(
        StereoFormat::FullSideBySide,
        EyeOrder::LeftRight,
        Eye::Left,
        3840,
        1080,
    )
    .unwrap();
    let fsbs_r = calculate_eye_pixel_rect(
        StereoFormat::FullSideBySide,
        EyeOrder::LeftRight,
        Eye::Right,
        3840,
        1080,
    )
    .unwrap();
    assert_eq!(
        fsbs_l,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(
        fsbs_r,
        StereoPixelRect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080
        }
    );

    // 8K Full SBS: 7680x4320 -> 3840x4320 each
    let fsbs_8k_l = calculate_eye_pixel_rect(
        StereoFormat::FullSideBySide,
        EyeOrder::LeftRight,
        Eye::Left,
        7680,
        4320,
    )
    .unwrap();
    let fsbs_8k_r = calculate_eye_pixel_rect(
        StereoFormat::FullSideBySide,
        EyeOrder::LeftRight,
        Eye::Right,
        7680,
        4320,
    )
    .unwrap();
    assert_eq!(
        fsbs_8k_l,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 3840,
            height: 4320
        }
    );
    assert_eq!(
        fsbs_8k_r,
        StereoPixelRect {
            x: 3840,
            y: 0,
            width: 3840,
            height: 4320
        }
    );

    // 1080p Half OU: 1920x1080 -> 1920x540 each
    let hou_l = calculate_eye_pixel_rect(
        StereoFormat::HalfOverUnder,
        EyeOrder::TopBottom,
        Eye::Left,
        1920,
        1080,
    )
    .unwrap();
    let hou_r = calculate_eye_pixel_rect(
        StereoFormat::HalfOverUnder,
        EyeOrder::TopBottom,
        Eye::Right,
        1920,
        1080,
    )
    .unwrap();
    assert_eq!(
        hou_l,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 540
        }
    );
    assert_eq!(
        hou_r,
        StereoPixelRect {
            x: 0,
            y: 540,
            width: 1920,
            height: 540
        }
    );
    assert_eq!(hou_l.height + hou_r.height, 1080);

    // 4K Full OU: 1920x2160 -> 1920x1080 each
    let fou_l = calculate_eye_pixel_rect(
        StereoFormat::FullOverUnder,
        EyeOrder::TopBottom,
        Eye::Left,
        1920,
        2160,
    )
    .unwrap();
    let fou_r = calculate_eye_pixel_rect(
        StereoFormat::FullOverUnder,
        EyeOrder::TopBottom,
        Eye::Right,
        1920,
        2160,
    )
    .unwrap();
    assert_eq!(
        fou_l,
        StereoPixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(
        fou_r,
        StereoPixelRect {
            x: 0,
            y: 1080,
            width: 1920,
            height: 1080
        }
    );
}

#[test]
fn test_odd_dimensions_rejection() {
    // SBS with odd width
    let sbs_odd = calculate_eye_pixel_rect(
        StereoFormat::HalfSideBySide,
        EyeOrder::LeftRight,
        Eye::Left,
        1919,
        1080,
    );
    assert!(sbs_odd.is_err());

    // OU with odd height
    let ou_odd = calculate_eye_pixel_rect(
        StereoFormat::HalfOverUnder,
        EyeOrder::TopBottom,
        Eye::Left,
        1920,
        1079,
    );
    assert!(ou_odd.is_err());

    // Zero dimensions
    assert!(
        calculate_eye_pixel_rect(StereoFormat::Mono, EyeOrder::LeftRight, Eye::Left, 0, 1080)
            .is_err()
    );
    assert!(
        calculate_eye_pixel_rect(StereoFormat::Mono, EyeOrder::LeftRight, Eye::Left, 1920, 0)
            .is_err()
    );
}

#[test]
fn test_display_aspect_ratio_accuracy() {
    let eps = 1e-4;

    // 16:9 standard ratio
    let standard_16_9 = 16.0 / 9.0;

    // 1920x1080 Half SBS
    assert!(
        (calculate_display_aspect_ratio(StereoFormat::HalfSideBySide, 1920, 1080) - standard_16_9)
            .abs()
            < eps
    );

    // 3840x1080 Full SBS
    assert!(
        (calculate_display_aspect_ratio(StereoFormat::FullSideBySide, 3840, 1080) - standard_16_9)
            .abs()
            < eps
    );

    // 1920x1080 Half OU
    assert!(
        (calculate_display_aspect_ratio(StereoFormat::HalfOverUnder, 1920, 1080) - standard_16_9)
            .abs()
            < eps
    );

    // 1920x2160 Full OU
    assert!(
        (calculate_display_aspect_ratio(StereoFormat::FullOverUnder, 1920, 2160) - standard_16_9)
            .abs()
            < eps
    );

    // 2.39:1 Cinemascope anamorphic half SBS (e.g. 1920x803)
    let cinemascope_pixel = 1920.0 / 803.0;
    assert!(
        (calculate_display_aspect_ratio(StereoFormat::HalfSideBySide, 1920, 803)
            - cinemascope_pixel)
            .abs()
            < eps
    );
}
