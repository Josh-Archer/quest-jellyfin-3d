use jellyfin_client::{
    ClientInfo, ItemsQuery, JellyfinClient, JellyfinError, PlaybackProgressRequest,
    PlaybackStartRequest, PlaybackStopRequest,
};
use url::Url;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_quick_connect_flow() {
    let mock_server = MockServer::start().await;
    let base_url = Url::parse(&mock_server.uri()).unwrap();
    let client = JellyfinClient::new(base_url, ClientInfo::default());

    // 1. Mock POST /QuickConnect/Initiate
    let initiate_json = serde_json::json!({
        "Secret": "qc-secret-12345",
        "Code": "842100",
        "Authenticated": false
    });
    Mock::given(method("POST"))
        .and(path("/QuickConnect/Initiate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&initiate_json))
        .mount(&mock_server)
        .await;

    let init_resp = client
        .initiate_quick_connect()
        .await
        .expect("QuickConnect Initiate succeeded");
    assert_eq!(init_resp.code, "842100");
    assert_eq!(init_resp.secret, "qc-secret-12345");
    assert!(!init_resp.authenticated);

    // 2. Mock GET /QuickConnect/Connect?secret=qc-secret-12345
    let connect_json = serde_json::json!({
        "Secret": "qc-secret-12345",
        "Code": "842100",
        "Authenticated": true
    });
    Mock::given(method("GET"))
        .and(path("/QuickConnect/Connect"))
        .and(query_param("secret", "qc-secret-12345"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&connect_json))
        .mount(&mock_server)
        .await;

    let state_resp = client
        .check_quick_connect("qc-secret-12345")
        .await
        .expect("QuickConnect state check succeeded");
    assert!(state_resp.authenticated);

    // 3. Mock POST /Users/AuthenticateWithQuickConnect
    let auth_json = serde_json::json!({
        "AccessToken": "jellyfin-token-xyz-789",
        "ServerId": "srv-homelab-01",
        "User": {
            "Id": "user-guid-josh-42",
            "Name": "josh"
        },
        "SessionInfo": {
            "Id": "sess-guid-999"
        }
    });
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateWithQuickConnect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&auth_json))
        .mount(&mock_server)
        .await;

    let auth_resp = client
        .authenticate_quick_connect("qc-secret-12345")
        .await
        .expect("Authentication succeeded");
    assert_eq!(auth_resp.access_token, "jellyfin-token-xyz-789");
    assert_eq!(auth_resp.user.name, "josh");

    // Verify client stored session credentials
    assert_eq!(
        client.token().await.as_deref(),
        Some("jellyfin-token-xyz-789")
    );
    assert_eq!(client.user_id().await.as_deref(), Some("user-guid-josh-42"));
}

#[tokio::test]
async fn test_browse_items_library() {
    let mock_server = MockServer::start().await;
    let base_url = Url::parse(&mock_server.uri()).unwrap();
    let client = JellyfinClient::new(base_url, ClientInfo::default());
    client.set_token("test-token-123").await;
    client.set_user_id("user-guid-1").await;

    let items_fixture: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/items_3d_response.json")).unwrap();

    Mock::given(method("GET"))
        .and(path("/Items"))
        .and(header("X-Emby-Token", "test-token-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&items_fixture))
        .mount(&mock_server)
        .await;

    let query = ItemsQuery::default();
    let resp = client
        .browse_items(&query)
        .await
        .expect("Browse items succeeded");

    assert_eq!(resp.items.len(), 8);
    assert_eq!(resp.total_record_count, 8);

    // Verify 3D format detection on returned items
    let avatar = &resp.items[0];
    assert_eq!(avatar.name, "Avatar (2009)");
    let detected = client.detect_stereo_format(avatar);
    assert!(detected.format.is_sbs());
}

#[tokio::test]
async fn test_playback_info_and_direct_play_url() {
    let mock_server = MockServer::start().await;
    let base_url = Url::parse(&mock_server.uri()).unwrap();
    let client = JellyfinClient::new(base_url, ClientInfo::default());
    client.set_token("token-abc").await;

    let playback_fixture: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/playback_info_response.json")).unwrap();

    Mock::given(method("POST"))
        .and(path("/Items/item-123/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&playback_fixture))
        .mount(&mock_server)
        .await;

    let pinfo = client
        .get_playback_info("item-123", None)
        .await
        .expect("PlaybackInfo succeeded");

    assert_eq!(
        pinfo.play_session_id.as_deref(),
        Some("play-session-quest-3d-888999")
    );
    assert_eq!(pinfo.media_sources.len(), 1);
    assert!(pinfo.media_sources[0].supports_direct_play);

    // Direct play stream URL
    let stream_url = client.direct_play_url("item-123").await.unwrap();
    assert!(stream_url
        .as_str()
        .contains("/Videos/item-123/stream?static=true"));
    assert!(stream_url.as_str().contains("api_key=token-abc"));
}

#[tokio::test]
async fn test_session_reporting_lifecycle() {
    let mock_server = MockServer::start().await;
    let base_url = Url::parse(&mock_server.uri()).unwrap();
    let client = JellyfinClient::new(base_url, ClientInfo::default());

    // 1. POST /Sessions/Playing
    Mock::given(method("POST"))
        .and(path("/Sessions/Playing"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let start_req = PlaybackStartRequest::direct_play("movie-1", Some("sess-1".to_string()), 0);
    client
        .report_playing(&start_req)
        .await
        .expect("Report playing succeeded");

    // 2. POST /Sessions/Playing/Progress
    Mock::given(method("POST"))
        .and(path("/Sessions/Playing/Progress"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let progress_req = PlaybackProgressRequest::time_update(
        "movie-1",
        Some("sess-1".to_string()),
        10_000_000,
        false,
    );
    client
        .report_progress(&progress_req)
        .await
        .expect("Report progress succeeded");

    // 3. POST /Sessions/Playing/Stopped
    Mock::given(method("POST"))
        .and(path("/Sessions/Playing/Stopped"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let stop_req = PlaybackStopRequest::new("movie-1", Some("sess-1".to_string()), 50_000_000);
    client
        .report_stopped(&stop_req)
        .await
        .expect("Report stopped succeeded");
}

#[tokio::test]
async fn test_error_scenarios() {
    let mock_server = MockServer::start().await;
    let base_url = Url::parse(&mock_server.uri()).unwrap();
    let client = JellyfinClient::new(base_url, ClientInfo::default());

    // 401 Unauthorized
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;

    let query = ItemsQuery::default();
    let err_401 = client.browse_items(&query).await.unwrap_err();
    assert!(matches!(err_401, JellyfinError::Unauthorized));

    // 404 Not Found
    Mock::given(method("GET"))
        .and(path("/Items/missing-id"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let err_404 = client.get_item("missing-id").await.unwrap_err();
    assert!(matches!(err_404, JellyfinError::NotFound(_)));

    // 500 Internal Server Error
    Mock::given(method("POST"))
        .and(path("/QuickConnect/Initiate"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal DB error"))
        .mount(&mock_server)
        .await;

    let err_500 = client.initiate_quick_connect().await.unwrap_err();
    match err_500 {
        JellyfinError::Api { status, message } => {
            assert_eq!(status, 500);
            assert!(message.contains("Internal DB error"));
        }
        other => panic!("Expected Api error, got {:?}", other),
    }
}
