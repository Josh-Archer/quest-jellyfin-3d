//! UI data contracts against authenticated Jellyfin routes.
use jellyfin_client::{StoredSession, TokenStore};
use quest_app::{bridge::run, AppSessionStore};
use serde_json::{json, Value};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Fixture {
    server: MockServer,
    dir: std::path::PathBuf,
    base: Value,
}
impl Fixture {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let dir = std::env::temp_dir().join(format!(
            "quest-library-ui-{}-{}",
            std::process::id(),
            server.address().port()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session.json");
        AppSessionStore(file.clone())
            .save(&StoredSession {
                server_url: server.uri(),
                access_token: "fixture-token".into(),
                user_id: "viewer".into(),
                user_name: "Viewer".into(),
                server_id: "server".into(),
                device_id: "quest".into(),
            })
            .unwrap();
        let base = json!({"server":server.uri(),"store":file,"device":"quest"});
        Self { server, dir, base }
    }
    async fn call(&self, action: &str, extra: Value) -> Value {
        let mut request = self.base.clone();
        request["action"] = json!(action);
        for (key, value) in extra.as_object().unwrap() {
            request[key] = value.clone();
        }
        run(request).await.unwrap()
    }
    async fn assert_authenticated(&self) {
        let requests = self.server.received_requests().await.unwrap();
        assert!(!requests.is_empty());
        for request in requests {
            assert!(request
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Token=\"fixture-token\""));
            assert!(!request.url.as_str().contains("fixture-token"));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn home_libraries_resume_and_poster_fallback_preserve_user_data() {
    let f = Fixture::new().await;
    Mock::given(method("GET"))
        .and(path("/Users/viewer/Views"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"Items":[{"Id":"movies", "Name":"Movies", "Type":"CollectionFolder"}]}),
        ))
        .expect(1)
        .mount(&f.server)
        .await;
    Mock::given(method("GET")).and(path("/Users/viewer/Items/Resume"))
        .and(query_param("MediaTypes", "Video"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Items":[{"Id":"episode","Name":"Pilot","Type":"Episode","SeriesId":"show","SeriesName":"Series","SeriesPrimaryImageTag":"series-art","ParentIndexNumber":1,"IndexNumber":2,"RunTimeTicks":3000000000_i64,"UserData":{"PlaybackPositionTicks":500000000,"IsFavorite":true}}]}))).expect(1).mount(&f.server).await;
    assert_eq!(f.call("status", json!({})).await["user"], "Viewer");
    assert_eq!(
        f.call("libraries", json!({})).await["items"][0]["id"],
        "movies"
    );
    let response = f.call("resume", json!({})).await;
    let item = &response["items"][0];
    assert_eq!(item["image_id"], "show");
    assert_eq!(item["image_tag"], "series-art");
    assert_eq!(item["resume"], 500000000);
    assert_eq!(item["favorite"], true);
    assert_eq!(item["season"], 1);
    assert_eq!(item["episode"], 2);
    f.assert_authenticated().await;
}

#[tokio::test]
async fn browse_filters_series_favorites_and_episode_order_on_the_server() {
    let f = Fixture::new().await;
    for (kind, expected_type) in [
        ("movies", "Movie"),
        ("shows", "Series"),
        ("episodes", "Episode"),
        ("favorites", "Movie,Series,Episode,Video"),
    ] {
        let mut mock = Mock::given(method("GET"))
            .and(path("/Items"))
            .and(query_param("IncludeItemTypes", expected_type))
            .and(query_param("ParentId", "parent"))
            .and(query_param("IsFavorite", "true"))
            .and(query_param("EnableImages", "true"));
        if kind == "episodes" {
            mock = mock
                .and(query_param(
                    "SortBy",
                    "ParentIndexNumber,IndexNumber,SortName",
                ))
                .and(query_param("SortOrder", "Ascending"));
        } else {
            mock = mock
                .and(query_param("SortBy", "DateCreated"))
                .and(query_param("SortOrder", "Descending"));
        }
        mock.respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Items":[],"TotalRecordCount":0})),
        )
        .expect(1)
        .mount(&f.server)
        .await;
        let response = f
            .call(
                "browse",
                json!({"kind":kind,"parent":"parent","favorite":true,"sort":"latest"}),
            )
            .await;
        assert!(response["items"].as_array().unwrap().is_empty());
        assert!(response["next"].is_null());
    }
    f.assert_authenticated().await;
}

#[tokio::test]
async fn details_favorites_and_jpeg_artwork_use_native_jellyfin_routes() {
    use base64::Engine;
    let f = Fixture::new().await;
    Mock::given(method("GET")).and(path("/Users/viewer/Items/movie"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"movie","Name":"Film","Overview":"Plot","ProductionYear":2026,"ImageTags":{"Primary":"poster-v2"}}))).mount(&f.server).await;
    Mock::given(method("GET"))
        .and(path("/Items/movie/Images/Primary"))
        .and(query_param("format", "Jpg"))
        .and(header("accept", "image/jpeg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![255, 216, 255, 217]))
        .expect(1)
        .mount(&f.server)
        .await;
    for verb in ["POST", "DELETE"] {
        Mock::given(method(verb))
            .and(path("/Users/viewer/FavoriteItems/movie"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"IsFavorite":verb=="POST"})),
            )
            .expect(1)
            .mount(&f.server)
            .await;
    }
    let detail = f.call("details", json!({"item":"movie"})).await;
    assert_eq!(detail["overview"], "Plot");
    assert_eq!(detail["image_id"], "movie");
    assert_eq!(detail["image_tag"], "poster-v2");
    let image = f.call("image", json!({"item":"movie"})).await;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(image["image"].as_str().unwrap())
            .unwrap(),
        vec![255, 216, 255, 217]
    );
    f.call("favorite", json!({"item":"movie", "favorite":true}))
        .await;
    f.call("favorite", json!({"item":"movie", "favorite":false}))
        .await;
    f.assert_authenticated().await;
}

#[tokio::test]
async fn playback_settings_reach_negotiation_and_refuse_disallowed_transcoding() {
    use wiremock::matchers::body_partial_json;
    let f = Fixture::new().await;
    Mock::given(method("GET"))
        .and(path("/Users/viewer/Items/movie"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"movie","Name":"Film"})))
        .mount(&f.server)
        .await;
    Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo"))
        .and(body_partial_json(json!({"MaxStreamingBitrate":40000000,"EnableTranscoding":false,"DeviceProfile":{"MaxStreamingBitrate":40000000}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"MediaSources":[{"Id":"source","SupportsTranscoding":true,"TranscodingUrl":"/Videos/movie/master.m3u8"}]}))).expect(1).mount(&f.server).await;
    let mut request = f.base.clone();
    request["action"] = json!("prepare");
    request["item"] = json!("movie");
    request["max_bitrate"] = json!(40000000);
    request["allow_transcoding"] = json!(false);
    assert!(run(request)
        .await
        .unwrap_err()
        .contains("requires server transcoding"));
    f.assert_authenticated().await;
}
