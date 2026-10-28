use jellyfin_client::{build_direct_play_url, ClientInfo, Session};
use url::Url;

#[test]
fn test_direct_play_url_generation() {
    let base_with_slash = Url::parse("https://jellyfin.example.com/").unwrap();
    let url1 = build_direct_play_url(&base_with_slash, "item-12345", None).unwrap();
    assert_eq!(
        url1.as_str(),
        "https://jellyfin.example.com/Videos/item-12345/stream?static=true"
    );

    let base_without_slash = Url::parse("https://jellyfin.example.com").unwrap();
    let url2 =
        build_direct_play_url(&base_without_slash, "item-12345", Some("secret_token")).unwrap();
    assert_eq!(
        url2.as_str(),
        "https://jellyfin.example.com/Videos/item-12345/stream?static=true&api_key=secret_token"
    );
}

#[test]
fn test_auth_headers_and_client_info() {
    let client = ClientInfo::new(
        "quest-jellyfin-3d",
        "Meta Quest 3",
        "device-guid-456",
        "0.1.0",
    );

    let header_no_token = client.authorization_header(None);
    assert_eq!(
        header_no_token,
        r#"MediaBrowser Client="quest-jellyfin-3d", Device="Meta Quest 3", DeviceId="device-guid-456", Version="0.1.0""#
    );

    let header_with_token = client.authorization_header(Some("tok-abc-123"));
    assert_eq!(
        header_with_token,
        r#"MediaBrowser Client="quest-jellyfin-3d", Device="Meta Quest 3", DeviceId="device-guid-456", Version="0.1.0", Token="tok-abc-123""#
    );
}

#[test]
fn test_session_lifecycle() {
    let mut session = Session::default();
    assert!(!session.is_authenticated());

    session.access_token = Some("valid_token".to_string());
    session.user_id = Some("user_123".to_string());
    assert!(session.is_authenticated());

    session.clear();
    assert!(!session.is_authenticated());
    assert_eq!(session.access_token, None);
    assert_eq!(session.user_id, None);
}
