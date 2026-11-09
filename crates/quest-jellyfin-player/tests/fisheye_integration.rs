//! Filename detection through the cinema projection choice. Aspect alone must not invent fisheye.
use jellyfin_client::BaseItem;
use quest_jellyfin_player::projection::{horizontal_angle, resolve_projection};
use quest_jellyfin_player::{detect_item_format, Projection};

#[test]
fn classified_fisheye_uses_a_hemisphere_and_a_wide_movie_stays_flat() {
    let dive = BaseItem {
        path: Some("/media/Reef.fisheye.mp4".into()),
        ..Default::default()
    };
    let detected = detect_item_format(&dive);
    assert_eq!(detected.projection, Projection::Fisheye);
    assert_eq!(
        resolve_projection("Auto", detected.projection, &[], 2.0),
        Projection::Fisheye
    );
    assert_eq!(
        horizontal_angle(Projection::Fisheye),
        Some(std::f32::consts::PI)
    );
    assert_ne!(
        horizontal_angle(Projection::Fisheye),
        horizontal_angle(Projection::Equirectangular360)
    );

    let wide = BaseItem {
        path: Some("/media/Lawrence of Arabia.mkv".into()),
        ..Default::default()
    };
    assert_eq!(detect_item_format(&wide).projection, Projection::Flat);
    assert_eq!(
        resolve_projection("Auto", Projection::Flat, &["widescreen".into()], 2.4),
        Projection::Flat
    );
    assert_eq!(horizontal_angle(Projection::Flat), None);
    assert_eq!(
        resolve_projection("Flat", Projection::Fisheye, &["fisheye".into()], 1.0),
        Projection::Flat,
        "an explicit flat choice overrides a fisheye tag"
    );
}
