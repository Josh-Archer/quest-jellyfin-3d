use jellyfin_client::{
    detect_from_container, detect_from_filename, detect_stereo_format, DetectionSource, EyeOrder,
    ItemsResponse, JellyfinVideo3DFormat, MediaItem, MediaStream, ProjectionType, StereoFormat,
    StreamType,
};

#[test]
fn test_fixture_items_format_detection() {
    let json_bytes = include_bytes!("fixtures/items_3d_response.json");
    let resp: ItemsResponse = serde_json::from_slice(json_bytes).expect("Valid items fixture");
    assert_eq!(resp.items.len(), 8);

    // Item 0: Avatar HSBS (Video3DFormat: HalfSideBySide)
    let avatar = &resp.items[0];
    let d0 = detect_stereo_format(avatar);
    assert_eq!(d0.format, StereoFormat::HalfSideBySide);
    assert_eq!(d0.eye_order, EyeOrder::LeftRight);
    assert_eq!(d0.projection, ProjectionType::FlatVirtualScreen);
    assert_eq!(d0.detection_source, DetectionSource::Jellyfin3DFormat);

    // Item 1: Space Walk FSBS (Video3DFormat: FullSideBySide)
    let space = &resp.items[1];
    let d1 = detect_stereo_format(space);
    assert_eq!(d1.format, StereoFormat::FullSideBySide);
    assert_eq!(d1.eye_order, EyeOrder::LeftRight);
    assert_eq!(d1.detection_source, DetectionSource::Jellyfin3DFormat);

    // Item 2: Gravity TAB (Video3DFormat: HalfTopAndBottom, tag _TB)
    let gravity = &resp.items[2];
    let d2 = detect_stereo_format(gravity);
    assert_eq!(d2.format, StereoFormat::HalfOverUnder);
    assert_eq!(d2.eye_order, EyeOrder::TopBottom);
    assert_eq!(d2.detection_source, DetectionSource::Jellyfin3DFormat);

    // Item 3: Rollercoaster VR180 HSBS (filename tags VR180, HSBS)
    let roller = &resp.items[3];
    let d3 = detect_stereo_format(roller);
    assert_eq!(d3.format, StereoFormat::HalfSideBySide);
    assert_eq!(d3.projection, ProjectionType::Vr180);
    assert_eq!(d3.detection_source, DetectionSource::FilenameTag);

    // Item 4: Wildlife Safari Fisheye (tags fisheye, SBS, _LR)
    let safari = &resp.items[4];
    let d4 = detect_stereo_format(safari);
    assert_eq!(d4.format, StereoFormat::HalfSideBySide);
    assert_eq!(d4.projection, ProjectionType::Fisheye);
    assert_eq!(d4.eye_order, EyeOrder::LeftRight);
    assert_eq!(d4.detection_source, DetectionSource::FilenameTag);

    // Item 5: Earth Orbit 360 FSBS (tags 360, FSBS)
    let earth = &resp.items[5];
    let d5 = detect_stereo_format(earth);
    assert_eq!(d5.format, StereoFormat::FullSideBySide);
    assert_eq!(d5.projection, ProjectionType::Vr360);
    assert_eq!(d5.detection_source, DetectionSource::FilenameTag);

    // Item 6: Inverted Eye RL (tag _RL, container 11)
    let inverted = &resp.items[6];
    let d6 = detect_stereo_format(inverted);
    assert_eq!(d6.format, StereoFormat::HalfSideBySide);
    assert_eq!(d6.eye_order, EyeOrder::RightLeft);
    assert_eq!(d6.detection_source, DetectionSource::FilenameTag);

    // Item 7: Normal 2D (Interstellar)
    let interstellar = &resp.items[7];
    let d7 = detect_stereo_format(interstellar);
    assert_eq!(d7.format, StereoFormat::Mono);
    assert_eq!(d7.detection_source, DetectionSource::FallbackDefault);
}

#[test]
fn test_all_filename_tags_individually() {
    let test_cases = [
        (
            "Demo.3D.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.HSBS.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.HALF-SBS.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.H-SBS.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.FSBS.mkv",
            StereoFormat::FullSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.FULL-SBS.mkv",
            StereoFormat::FullSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.OU.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.HOU.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.HALF-OU.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.FOU.mkv",
            StereoFormat::FullOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.FULL-OU.mkv",
            StereoFormat::FullOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.TAB.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.HTAB.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.FTAB.mkv",
            StereoFormat::FullOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.VR180.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::Vr180,
        ),
        (
            "Demo.180.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::Vr180,
        ),
        (
            "Demo.VR360.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::Vr360,
        ),
        (
            "Demo.360.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::Vr360,
        ),
        (
            "Demo.fisheye.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::Fisheye,
        ),
        (
            "Demo.3D._LR.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.3D._RL.mkv",
            StereoFormat::HalfSideBySide,
            EyeOrder::RightLeft,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.TAB._TB.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
            ProjectionType::FlatVirtualScreen,
        ),
        (
            "Demo.TAB._BT.mkv",
            StereoFormat::HalfOverUnder,
            EyeOrder::BottomTop,
            ProjectionType::FlatVirtualScreen,
        ),
    ];

    for (filename, expected_fmt, expected_eye, expected_proj) in test_cases {
        let detected = detect_from_filename(filename)
            .unwrap_or_else(|| panic!("Expected detection for filename '{}'", filename));
        assert_eq!(
            detected.format, expected_fmt,
            "Fmt mismatch in {}",
            filename
        );
        assert_eq!(
            detected.eye_order, expected_eye,
            "Eye mismatch in {}",
            filename
        );
        assert_eq!(
            detected.projection, expected_proj,
            "Proj mismatch in {}",
            filename
        );
        assert_eq!(detected.detection_source, DetectionSource::FilenameTag);
    }
}

#[test]
fn test_video_3d_format_overrides_filename_and_container() {
    let item = MediaItem {
        id: "override-test".to_string(),
        name: "Conflicting.FSBS.VR180.mkv".to_string(),
        original_title: None,
        path: Some("/media/Conflicting.FSBS.VR180.mkv".to_string()),
        video_3d_format: Some(JellyfinVideo3DFormat::HalfTopAndBottom), // Video3DFormat takes priority
        media_streams: vec![MediaStream {
            codec: Some("hevc".to_string()),
            stream_type: Some(StreamType::Video),
            stereo_mode: Some("1".to_string()), // Container says SBS
            ..Default::default()
        }],
        container: Some("mkv".to_string()),
        run_time_ticks: None,
        overview: None,
        item_type: Some("Movie".to_string()),
        user_data: None,
    };

    let detected = detect_stereo_format(&item);
    assert_eq!(detected.format, StereoFormat::HalfOverUnder);
    assert_eq!(detected.detection_source, DetectionSource::Jellyfin3DFormat);
}

#[test]
fn test_container_metadata_variations() {
    let cases = [
        ("1", StereoFormat::HalfSideBySide, EyeOrder::LeftRight),
        (
            "side_by_side",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
        ),
        (
            "left_right",
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
        ),
        ("11", StereoFormat::HalfSideBySide, EyeOrder::RightLeft),
        (
            "right_left",
            StereoFormat::HalfSideBySide,
            EyeOrder::RightLeft,
        ),
        ("3", StereoFormat::HalfOverUnder, EyeOrder::TopBottom),
        (
            "top_bottom",
            StereoFormat::HalfOverUnder,
            EyeOrder::TopBottom,
        ),
        ("2", StereoFormat::HalfOverUnder, EyeOrder::BottomTop),
        (
            "bottom_top",
            StereoFormat::HalfOverUnder,
            EyeOrder::BottomTop,
        ),
        ("0", StereoFormat::Mono, EyeOrder::LeftRight),
        ("mono", StereoFormat::Mono, EyeOrder::LeftRight),
    ];

    for (mode_str, expected_fmt, expected_eye) in cases {
        let item = MediaItem {
            id: "c-test".to_string(),
            name: "UnmarkedMovie.mkv".to_string(),
            original_title: None,
            path: Some("/media/UnmarkedMovie.mkv".to_string()),
            video_3d_format: None,
            media_streams: vec![MediaStream {
                codec: Some("hevc".to_string()),
                stream_type: Some(StreamType::Video),
                stereo_mode: Some(mode_str.to_string()),
                ..Default::default()
            }],
            container: Some("mkv".to_string()),
            run_time_ticks: None,
            overview: None,
            item_type: Some("Movie".to_string()),
            user_data: None,
        };

        let detected = detect_from_container(&item)
            .unwrap_or_else(|| panic!("Expected container detection for mode '{}'", mode_str));
        assert_eq!(
            detected.format, expected_fmt,
            "Fmt mismatch for mode {}",
            mode_str
        );
        assert_eq!(
            detected.eye_order, expected_eye,
            "Eye mismatch for mode {}",
            mode_str
        );
        assert_eq!(
            detected.detection_source,
            DetectionSource::ContainerMetadata
        );
    }
}
