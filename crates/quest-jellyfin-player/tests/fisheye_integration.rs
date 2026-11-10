//! Filename detection through the cinema projection choice. Aspect alone must not invent fisheye.
//! Classified fisheye resolves to the equidistant UV remap, not a flat quad or plain VR180.
use jellyfin_client::BaseItem;
use quest_jellyfin_player::fisheye::{
    spherical_submission, EquidistantFisheye, FisheyeRemapMesh, SphericalSubmission,
};
use quest_jellyfin_player::projection::{horizontal_angle, resolve_projection};
use quest_jellyfin_player::{detect_item_format, Projection};

#[test]
fn classified_fisheye_uses_equidistant_remap_and_a_wide_movie_stays_flat() {
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
        spherical_submission(Projection::Fisheye),
        Some(SphericalSubmission::FisheyeEquidistantRemap)
    );
    assert_ne!(
        spherical_submission(Projection::Fisheye),
        spherical_submission(Projection::Equirectangular180),
        "fisheye must not collapse to plain VR180 equirect"
    );
    assert_ne!(
        spherical_submission(Projection::Fisheye),
        None,
        "fisheye must not flatten onto a quad"
    );
    assert_eq!(
        horizontal_angle(Projection::Fisheye),
        Some(std::f32::consts::PI),
        "hemispherical carrier coverage for the remap path"
    );
    let mesh = FisheyeRemapMesh::hemisphere(EquidistantFisheye::default(), 8, 16);
    assert!(
        !mesh.vertices.is_empty() && !mesh.indices.is_empty(),
        "equidistant remap mesh must be built for classified fisheye"
    );
    assert_eq!(mesh.lens, EquidistantFisheye::default());

    let wide = BaseItem {
        path: Some("/media/Lawrence of Arabia.mkv".into()),
        ..Default::default()
    };
    assert_eq!(detect_item_format(&wide).projection, Projection::Flat);
    assert_eq!(
        resolve_projection("Auto", Projection::Flat, &["widescreen".into()], 2.4),
        Projection::Flat
    );
    assert_eq!(spherical_submission(Projection::Flat), None);
    assert_eq!(horizontal_angle(Projection::Flat), None);
    assert_eq!(
        resolve_projection("Flat", Projection::Fisheye, &["fisheye".into()], 1.0),
        Projection::Flat,
        "an explicit flat choice overrides a fisheye tag"
    );
}

#[test]
fn jellyfin_fisheye_tag_selects_remap_not_vr180() {
    assert_eq!(
        resolve_projection("Auto", Projection::Flat, &["Fisheye".into()], 1.0),
        Projection::Fisheye
    );
    assert_eq!(
        spherical_submission(resolve_projection(
            "Auto",
            Projection::Flat,
            &["Fisheye".into()],
            1.0
        )),
        Some(SphericalSubmission::FisheyeEquidistantRemap)
    );
    assert_eq!(
        spherical_submission(resolve_projection(
            "Auto",
            Projection::Flat,
            &["VR180".into()],
            1.0
        )),
        Some(SphericalSubmission::Equirect180)
    );
}
