use quest_app::bridge::run;
use serde_json::{json, Value};
use wiremock::matchers::{body_partial_json, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn native_app_pairs_browses_prepares_and_reports_via_jellyfin_api() {
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-flow-{}-{}",
        std::process::id(),
        server.address().port()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let session_file = dir.join("session.json");
    let base = json!({"server":server.uri(), "device":"quest-test", "store":session_file.to_str().unwrap()});
    async fn call(base: &Value, action: &str, extra: Value) -> Value {
        let mut request = base.clone();
        request["action"] = json!(action);
        for (key, value) in extra.as_object().unwrap() {
            request[key] = value.clone();
        }
        run(request).await.unwrap()
    }
    Mock::given(method("POST"))
        .and(path("/QuickConnect/Initiate"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Secret":"secret", "Code":"123456"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/QuickConnect/Connect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Authenticated":true})))
        .mount(&server)
        .await;
    Mock::given(method("POST")).and(path("/Users/AuthenticateWithQuickConnect")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"AccessToken":"token", "ServerId":"server", "User":{"Id":"user", "Name":"viewer"}}))).mount(&server).await;
    Mock::given(method("GET")).and(path("/Items")).and(header_exists("authorization")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"Items":[{"Id":"movie", "Name":"Film", "Video3DFormat":"HalfSideBySide"}], "TotalRecordCount":1}))).mount(&server).await;
    Mock::given(method("GET")).and(path("/Users/user/Items/movie")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"movie", "Name":"Film", "Video3DFormat":"HalfSideBySide", "UserData":{"PlaybackPositionTicks":42000000}}))).mount(&server).await;
    Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo")).and(wiremock::matchers::body_partial_json(json!({"DeviceProfile":{"Name":"Meta Quest 3","CodecProfiles":[{"Codec":"hevc","Conditions":[{"Property":"Width","Value":"8192"},{"Property":"Height","Value":"4320"},{"Property":"VideoFramerate","Value":"30"}]}]}}))).respond_with(ResponseTemplate::new(200).set_body_json(json!({"PlaySessionId":"session", "MediaSources":[{"Id":"source", "SupportsDirectPlay":true}]}))).mount(&server).await;
    for path_value in [
        "/Sessions/Playing",
        "/Sessions/Playing/Progress",
        "/Sessions/Playing/Stopped",
    ] {
        Mock::given(method("POST")).and(path(path_value)).and(body_partial_json(json!({"ItemId":"movie", "MediaSourceId":"source", "PlaySessionId":"session", "PositionTicks":42000000}))).respond_with(ResponseTemplate::new(204)).expect(1).mount(&server).await;
    }
    assert_eq!(call(&base, "status", json!({})).await["signed_in"], false);
    assert_eq!(call(&base, "initiate", json!({})).await["code"], "123456");
    assert_eq!(
        call(&base, "poll", json!({"secret":"secret"})).await["authorized"],
        true
    );
    assert_eq!(call(&base, "status", json!({})).await["signed_in"], true);
    let page = call(&base, "browse", json!({})).await;
    assert_eq!(page["items"][0]["format"], "HalfSideBySide");
    let prepared = call(&base, "prepare", json!({"item":"movie"})).await;
    assert_eq!(prepared["resume"], 42000000);
    assert_eq!(prepared["format"], "HalfSideBySide");
    assert_eq!(
        prepared["reason"],
        "Jellyfin confirmed this source supports direct play"
    );
    assert!(!prepared["url"].as_str().unwrap().contains("token"));
    assert!(prepared["authorization"]
        .as_str()
        .unwrap()
        .contains("token"));
    let report = json!({"item":"movie", "source":"source", "session":"session", "ticks":42000000});
    for action in ["start", "progress", "stop"] {
        call(&base, action, report.clone()).await;
    }
    call(&base, "signout", json!({})).await;
    assert!(!session_file.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn transcode_uses_server_url_and_reports_correct_play_method() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-transcode-{}-{}",
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
            server_id: "s".into(),
            device_id: "quest".into(),
        })
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/Users/user/Items/movie"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Id":"movie", "Name":"Movie"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"PlaySessionId":"play", "MediaSources":[{"Id":"source", "SupportsTranscoding":true, "TranscodingUrl":"/Videos/movie/master.m3u8?api_key=token&x=1"}]}))).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/Sessions/Playing"))
        .and(body_partial_json(
            json!({"PlayMethod":"Transcode", "MediaSourceId":"source"}),
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let base = json!({"server":server.uri(),"device":"quest","store":file.to_str().unwrap(),"item":"movie"});
    let mut request = base.clone();
    request["action"] = json!("prepare");
    let prepared = run(request).await.unwrap();
    assert_eq!(prepared["method"], "Transcode");
    assert_eq!(
        prepared["reason"],
        "Jellyfin selected transcoding because direct play is unavailable"
    );
    assert!(!prepared["url"].as_str().unwrap().contains("token"));
    let mut report = base.clone();
    report["action"] = json!("start");
    report["method"] = prepared["method"].clone();
    report["source"] = prepared["source"].clone();
    report["session"] = prepared["session"].clone();
    run(report).await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn quest_decode_cap_overrides_server_direct_play_and_reports_why() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-quest-cap-{}-{}",
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
            server_id: "s".into(),
            device_id: "quest".into(),
        })
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/Users/user/Items/movie"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Id":"movie","Name":"Movie"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/Items/movie/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"MediaSources":[{"Id":"source","SupportsDirectPlay":true,"SupportsTranscoding":true,"TranscodingUrl":"/Videos/movie/master.m3u8","MediaStreams":[{"Type":"Video","Codec":"hevc","Width":7680,"Height":4320,"RealFrameRate":60.0}]}]})))
        .mount(&server)
        .await;
    let prepared = run(json!({"action":"prepare","server":server.uri(),"device":"quest","store":file.to_str().unwrap(),"item":"movie"})).await.unwrap();
    assert_eq!(prepared["method"], "Transcode");
    assert_eq!(
        prepared["url"],
        format!("{}/Videos/movie/master.m3u8", server.uri())
    );
    assert!(prepared["reason"]
        .as_str()
        .unwrap()
        .contains("8K30 hardware cap"));
    let direct = run(json!({"action":"prepare","server":server.uri(),"device":"quest","store":file.to_str().unwrap(),"item":"movie","hardware_hevc_8k60":true})).await.unwrap();
    assert_eq!(direct["method"], "DirectPlay");
    assert_eq!(direct["width"], 7680);
    assert_eq!(direct["height"], 4320);
    let requests = server.received_requests().await.unwrap();
    let sent = requests
        .iter()
        .rev()
        .find(|r| r.url.path().ends_with("PlaybackInfo"))
        .unwrap()
        .body_json::<Value>()
        .unwrap();
    let codec = sent["DeviceProfile"]["CodecProfiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Codec"] == "hevc")
        .unwrap();
    assert!(codec["Conditions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["Property"] == "VideoFramerate" && c["Value"] == "60"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn library_search_passes_the_term_and_page_to_jellyfin() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    use wiremock::matchers::query_param;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-search-{}-{}",
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
            server_id: "s".into(),
            device_id: "quest".into(),
        })
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/Items"))
        .and(query_param("SearchTerm", "Avatar"))
        .and(query_param(
            "IncludeItemTypes",
            "Movie,Series,Episode,Video",
        ))
        .and(query_param("StartIndex", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"Items":[{"Id":"one","Name":"Avatar"}],"StartIndex":50,"TotalRecordCount":52}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let response = run(json!({"action":"browse","server":server.uri(),"device":"quest","store":file.to_str().unwrap(),"search":" Avatar ","start":50})).await.unwrap();
    assert_eq!(response["items"][0]["name"], "Avatar");
    assert_eq!(response["next"], 51);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn mvhevc_without_packed_alternate_is_refused_naming_decode_path() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-mvhevc-{}-{}",
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
            server_id: "s".into(),
            device_id: "quest".into(),
        })
        .unwrap();
    Mock::given(method("GET")).and(path("/Users/user/Items/movie")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"movie","Name":"Movie", "MediaStreams":[{"Type":"Video","Codec":"hevc","Width":1920,"Height":1080}]}))).mount(&server).await;
    Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"MediaSources":[{"Id":"source", "SupportsDirectPlay":true,"Path":"/media/Spatial.MV-HEVC.mp4","MediaStreams":[{"Type":"Video","Codec":"mv-hevc","Width":1920,"Height":1080}]}]}))).mount(&server).await;
    let err = run(json!({"action":"prepare","server":server.uri(),"device":"quest","store":file.to_str().unwrap(),"item":"movie"})).await.unwrap_err();
    assert!(
        err.contains("dual-view MV-HEVC decode path") && err.contains("packed SBS/OU"),
        "refusal must name the missing decode path: {err}"
    );
    // Detection must still classify the stream as MvHevc, never ordinary HEVC/SBS.
    assert_eq!(
        quest_jellyfin_player::detect_item_format(&jellyfin_client::BaseItem {
            path: Some("/media/Spatial.MV-HEVC.mp4".into()),
            media_streams: vec![jellyfin_client::MediaStream {
                codec: Some("mv-hevc".into()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .format,
        quest_jellyfin_player::StereoFormat::MvHevc
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn mvhevc_uses_packed_alternate_when_jellyfin_offers_one() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-mvhevc-alt-{}-{}",
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
    Mock::given(method("GET"))
        .and(path("/Users/user/Items/movie"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Id":"movie","Name":"Spatial movie",
            "Path":"/movies/Movie.MV-HEVC.mov",
            "MediaStreams":[{"Type":"Video","Codec":"mv-hevc","Width":1920,"Height":1080}]
        })))
        .mount(&server)
        .await;
    let packed = json!({
        "Id":"packed","Path":"/movies/Movie - 3D.HSBS.mkv",
        "SupportsDirectPlay":true,
        "MediaStreams":[{"Type":"Video","Codec":"hevc","Width":3840,"Height":1080}]
    });
    Mock::given(method("POST"))
        .and(path("/Items/movie/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "MediaSources":[
                {"Id":"original","Path":"/movies/Movie.MV-HEVC.mov","SupportsDirectPlay":true,
                 "MediaStreams":[{"Type":"Video","Codec":"mv-hevc","Width":1920,"Height":1080}]},
                packed
            ]
        })))
        .mount(&server)
        .await;
    let prepared = run(json!({
        "action":"prepare","server":server.uri(),"device":"quest",
        "store":file.to_str().unwrap(),"item":"movie"
    }))
    .await
    .unwrap();
    assert_eq!(prepared["source"], "packed");
    assert_eq!(prepared["method"], "DirectPlay");
    assert_eq!(prepared["format"], "HalfSideBySide");
    assert_ne!(prepared["format"], "MvHevc");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn mvc_uses_packed_alternate_even_when_original_is_direct_playable() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-spatial-{}-{}",
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
    Mock::given(method("GET")).and(path("/Users/user/Items/movie"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"movie","Name":"Stereo movie","Video3DFormat":"MVC","Path":"/movies/Movie.3D.MVC.m2ts","Tags":["Equirectangular"]}))).mount(&server).await;
    let base = json!({"action":"prepare","server":server.uri(),"device":"quest","store":file.to_str().unwrap(),"item":"movie"});
    let source = json!({"Id":"packed","Path":"/movies/Movie - 3D.FSBS.mkv","SupportsTranscoding":true,"TranscodingUrl":"/Videos/movie/master.m3u8","MediaStreams":[{"Type":"Video","Codec":"hevc","Width":3840,"Height":1080}]});
    let guard = Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"MediaSources":[{"Id":"original","Path":"/movies/Movie.3D.MVC.m2ts","SupportsDirectPlay":true}, source]}))).mount_as_scoped(&server).await;
    let prepared = run(base.clone()).await.unwrap();
    // Merged versions may make the packed version primary; the original still must not win.
    Mock::given(method("GET")).and(path("/Users/user/Items/movie2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"movie2","Name":"Stereo movie","Video3DFormat":"FullSideBySide","Path":"/movies/Movie - 3D.FSBS.mkv"}))).mount(&server).await;
    Mock::given(method("POST")).and(path("/Items/movie2/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"MediaSources":[{"Id":"original","Path":"/movies/Movie.3D.MVC.m2ts","SupportsDirectPlay":true}, source]}))).mount(&server).await;
    let mut primary = base.clone();
    primary["item"] = json!("movie2");
    assert_eq!(run(primary).await.unwrap()["source"], "packed");
    assert_eq!(prepared["source"], "packed");
    assert_eq!(prepared["method"], "Transcode");
    assert_eq!(prepared["format"], "FullSideBySide");
    assert_eq!(prepared["projection"], "Equirectangular360");
    let mut flat = base.clone();
    flat["projection"] = json!("Flat");
    assert_eq!(run(flat).await.unwrap()["projection"], "Flat");
    let mut forbid = base.clone();
    forbid["allow_transcoding"] = json!(false);
    assert!(run(forbid)
        .await
        .unwrap_err()
        .contains("requires server transcoding"));
    drop(guard);
    Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"MediaSources":[{"Id":"original","Path":"/movies/Movie.3D.MVC.m2ts","SupportsDirectPlay":true,"MediaStreams":[{"Type":"Video","Codec":"h264","Width":1920,"Height":1080}]}]}))).mount(&server).await;
    assert_eq!(run(base).await.unwrap()["format"], "Mvc");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn prepare_hands_cinema_every_subtitle_stream_without_leaking_the_token() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-subs-{}-{}",
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
    Mock::given(method("GET"))
        .and(path("/Users/user/Items/movie"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Id": "movie",
            "Name": "Dialogue",
            "Path": "/movies/Dialogue.mkv"
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/Items/movie/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "PlaySessionId": "session",
            "MediaSources": [{
                "Id": "source",
                "Path": "/movies/Dialogue.mkv",
                "SupportsDirectPlay": true,
                "MediaStreams": [
                    {"Type": "Video", "Codec": "hevc", "Width": 1920, "Height": 1080, "Index": 0},
                    {"Type": "Subtitle", "Index": 2, "Codec": "subrip", "Language": "eng", "DisplayTitle": "English", "IsDefault": true},
                    {"Type": "Subtitle", "Index": 3, "Codec": "pgssub", "Language": "eng", "DisplayTitle": "English PGS", "IsExternal": false},
                    {"Type": "Subtitle", "Index": 4, "Codec": "ass", "Language": "spa", "Title": "Spanish", "IsExternal": true, "IsForced": true}
                ]
            }]
        })))
        .mount(&server)
        .await;
    let prepared = run(json!({
        "action": "prepare",
        "server": server.uri(),
        "device": "quest",
        "store": file.to_str().unwrap(),
        "item": "movie"
    }))
    .await
    .unwrap();
    let url = prepared["url"].as_str().unwrap();
    assert!(!url.contains("token"));
    assert!(prepared["authorization"]
        .as_str()
        .unwrap()
        .contains("token"));
    let subs = prepared["subtitles"].as_array().unwrap();
    assert_eq!(subs.len(), 3);
    assert_eq!(subs[0]["index"], 2);
    assert_eq!(subs[0]["codec"], "subrip");
    assert_eq!(subs[0]["language"], "eng");
    assert_eq!(subs[0]["is_default"], true);
    assert_eq!(subs[1]["codec"], "pgssub");
    assert_eq!(subs[2]["codec"], "ass");
    assert_eq!(subs[2]["is_forced"], true);
    assert_eq!(subs[2]["external"], true);
    assert_eq!(subs[2]["title"], "Spanish");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn prepare_uses_bumped_frame_rate_caps_without_leaking_token() {
    use jellyfin_client::{StoredSession, TokenStore};
    use quest_app::AppSessionStore;
    let server = MockServer::start().await;
    let dir = std::env::temp_dir().join(format!(
        "jellyfin-caps-{}-{}",
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
            server_id: "s".into(),
            device_id: "quest".into(),
        })
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/Users/user/Items/too-fast"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Id":"too-fast","Name":"8K60"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/Items/too-fast/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "PlaySessionId": "play-8k",
            "MediaSources": [{
                "Id": "source-8k",
                "SupportsDirectPlay": true,
                "SupportsTranscoding": true,
                "TranscodingUrl": "/Videos/too-fast/master.m3u8?api_key=token",
                "MediaStreams": [{
                    "Type": "Video",
                    "Codec": "hevc",
                    "Width": 7680,
                    "Height": 4320,
                    "RealFrameRate": 60.0
                }]
            }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Users/user/Items/hd"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"hd","Name":"1080p"})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/Items/hd/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "PlaySessionId": "play-hd",
            "MediaSources": [{
                "Id": "source-hd",
                "SupportsDirectPlay": true,
                "MediaStreams": [{
                    "Type": "Video",
                    "Codec": "h264",
                    "Width": 1920,
                    "Height": 1080,
                    "RealFrameRate": 24.0
                }]
            }]
        })))
        .mount(&server)
        .await;

    let too_fast = run(json!({
        "action": "prepare",
        "server": server.uri(),
        "device": "quest",
        "store": file.to_str().unwrap(),
        "item": "too-fast"
    }))
    .await
    .unwrap();
    assert_eq!(too_fast["method"], "Transcode");
    assert!(too_fast["reason"]
        .as_str()
        .unwrap()
        .contains("8K30 hardware cap"));
    assert!(!too_fast["url"].as_str().unwrap().contains("token"));

    let hd = run(json!({
        "action": "prepare",
        "server": server.uri(),
        "device": "quest",
        "store": file.to_str().unwrap(),
        "item": "hd"
    }))
    .await
    .unwrap();
    assert_eq!(hd["method"], "DirectPlay");
    assert_eq!(
        hd["reason"],
        "Jellyfin confirmed this source supports direct play"
    );
    assert!(!hd["url"].as_str().unwrap().contains("token"));
    assert!(hd["authorization"].as_str().unwrap().contains("token"));
    std::fs::remove_dir_all(dir).unwrap();
}
