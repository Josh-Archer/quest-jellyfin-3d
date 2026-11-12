//! Prepare a classified fisheye item, and keep raw MVC off the compositor.
use quest_app::bridge::run;
use quest_app::surface_size::{parse_projection, SurfaceSize};
use quest_jellyfin_player::fisheye::{spherical_submission, SphericalSubmission};
use quest_jellyfin_player::{EyeOrder, Projection, StereoFormat};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn fisheye_is_a_real_projection_and_raw_mvc_still_cannot_fill_the_screen() {
    assert_eq!(parse_projection("Fisheye").unwrap(), Projection::Fisheye);
    assert_eq!(
        spherical_submission(Projection::Fisheye),
        Some(SphericalSubmission::FisheyeEquidistantRemap)
    );
    assert_ne!(
        spherical_submission(Projection::Fisheye),
        spherical_submission(Projection::Equirectangular180)
    );
    SurfaceSize::new(3840, 1920, StereoFormat::Mono, EyeOrder::LeftRight)
        .expect("a decoded fisheye frame has dimensions the compositor can take");
    let err = SurfaceSize::new(1920, 1080, StereoFormat::Mvc, EyeOrder::LeftRight).unwrap_err();
    assert!(
        err.contains("packing") || err.contains("decoder"),
        "raw MVC must stay refused: {err}"
    );
    assert!(parse_projection("MvHevc").is_err());
}

#[tokio::test]
async fn prepare_reports_fisheye_for_a_tagged_file_and_flat_for_a_wide_movie() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-fisheye-{}-{}",
        std::process::id(),
        server.address().port()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session.json");
    AppSessionStore(file.clone())
        .save(&StoredSession {
            server_url: server.uri(),
            access_token: "token".into(),
            user_id: "user".into(),
            user_name: "viewer".into(),
            server_id: "server".into(),
            device_id: "quest".into(),
        })
        .unwrap();

    async fn prepare(
        server: &MockServer,
        file: &std::path::Path,
        id: &str,
        item_path: &str,
        width: i32,
        height: i32,
    ) -> serde_json::Value {
        Mock::given(method("GET"))
            .and(path(format!("/Users/user/Items/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Id": id,
                "Name": id,
                "Path": item_path
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/Items/{id}/PlaybackInfo")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "PlaySessionId": "session",
                "MediaSources": [{
                    "Id": "source",
                    "Path": item_path,
                    "SupportsDirectPlay": true,
                    "MediaStreams": [{
                        "Type": "Video",
                        "Codec": "hevc",
                        "Width": width,
                        "Height": height
                    }]
                }]
            })))
            .mount(server)
            .await;
        run(json!({
            "action": "prepare",
            "server": server.uri(),
            "device": "quest",
            "store": file.to_str().unwrap(),
            "item": id
        }))
        .await
        .unwrap()
    }

    let fisheye = prepare(
        &server,
        &file,
        "reef",
        "/movies/Reef.fisheye.mp4",
        3840,
        1920,
    )
    .await;
    assert_eq!(fisheye["projection"], "Fisheye");
    assert_ne!(fisheye["format"], "Mvc");
    assert_ne!(fisheye["format"], "MvHevc");
    assert!(!fisheye["url"].as_str().unwrap().contains("token"));

    let wide = prepare(
        &server,
        &file,
        "lawrence",
        "/movies/Lawrence of Arabia.mkv",
        3840,
        1600,
    )
    .await;
    assert_eq!(wide["projection"], "Flat");
    std::fs::remove_dir_all(dir).unwrap();
}
