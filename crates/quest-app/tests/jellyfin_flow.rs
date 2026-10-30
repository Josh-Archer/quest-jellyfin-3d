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
    Mock::given(method("POST")).and(path("/Items/movie/PlaybackInfo")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"PlaySessionId":"session", "MediaSources":[{"Id":"source", "SupportsDirectPlay":true}]}))).mount(&server).await;
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
