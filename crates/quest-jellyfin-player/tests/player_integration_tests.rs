use std::sync::Arc;

use jellyfin_client::{
    BaseItem, ClientIdentity, JellyfinClient, MediaStream, MediaStreamType, MemoryTokenStore,
    StoredSession, TokenStore, Video3DFormat,
};
use quest_jellyfin_player::{PlaybackEngine, PlaybackState, StereoFormat};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use xr_video::config::VideoCodec;
use xr_video::decoder::mock::MockHardwareDecoder;

#[tokio::test]
async fn test_playback_engine_full_lifecycle() {
    let mock_server = MockServer::start().await;
    let store = Arc::new(MemoryTokenStore::new());
    store
        .save(&StoredSession {
            server_url: mock_server.uri(),
            access_token: "test-quest-token".to_owned(),
            user_id: "user-1".to_owned(),
            user_name: "josh".to_owned(),
            server_id: "server-1".to_owned(),
            device_id: "quest-3".to_owned(),
        })
        .unwrap();
    let identity = ClientIdentity::new("quest-jellyfin-3d", "Quest 3", "quest-3", "0.1.0").unwrap();
    let client = JellyfinClient::new(&mock_server.uri(), identity, store).unwrap();

    // 1. Mock PlaybackInfo
    let pinfo_json = serde_json::json!({
        "PlaySessionId": "sess-integrated-12345",
        "MediaSources": [
            {
                "Id": "source-1",
                "Container": "mkv",
                "SupportsDirectPlay": true,
                "SupportsDirectStream": true,
                "SupportsTranscoding": false,
                "Bitrate": 35000000
            }
        ]
    });
    Mock::given(method("POST"))
        .and(path("/Items/item-avatar-3d/PlaybackInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&pinfo_json))
        .mount(&mock_server)
        .await;

    // 2. Mock Session endpoints
    Mock::given(method("POST"))
        .and(path("/Sessions/Playing"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/Sessions/Playing/Progress"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/Sessions/Playing/Stopped"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock_server)
        .await;

    let decoder = Box::new(MockHardwareDecoder::new(3840, 1080, VideoCodec::Hevc));
    let mut engine = PlaybackEngine::new(client, decoder);
    assert_eq!(engine.state(), PlaybackState::Idle);

    let item = BaseItem {
        id: "item-avatar-3d".to_string(),
        name: "Avatar 3D FSBS".to_string(),
        path: Some("/media/3D/Avatar.FSBS.1080p.mkv".to_string()),
        video_3d_format: Some(Video3DFormat::FullSideBySide),
        media_streams: vec![MediaStream {
            codec: Some("hevc".to_string()),
            stream_type: Some(MediaStreamType::Video),
            width: Some(3840),
            height: Some(1080),
            real_frame_rate: Some(24.0),
            ..Default::default()
        }],
        container: Some("mkv".to_string()),
        run_time_ticks: Some(90_000_000_000),
        item_type: Some("Movie".to_string()),
        ..Default::default()
    };

    // Load item
    engine
        .load_item(item)
        .await
        .expect("Item loaded successfully");
    assert_eq!(
        engine.detected_format().unwrap().format,
        StereoFormat::FullSideBySide
    );
    assert!(engine
        .direct_play_url()
        .unwrap()
        .as_str()
        .contains("static=true"));

    // Start playback
    engine.play().await.expect("Playback started");
    assert_eq!(engine.state(), PlaybackState::Playing);

    // Queue packets and decode frames
    let packet_data = vec![0x00, 0x00, 0x00, 0x01, 0x26]; // NALU header
    engine
        .queue_packet(&packet_data, 0, true)
        .expect("Packet queued");
    engine
        .queue_packet(&packet_data, 41666, false)
        .expect("Packet queued");

    let frame = engine
        .dequeue_frame(1000)
        .expect("Frame dequeued")
        .expect("Decoded frame present");
    assert_eq!(frame.width, 3840);
    assert_eq!(frame.height, 1080);
    assert!(frame.is_keyframe);
    engine
        .release_frame(frame.buffer_index, true)
        .expect("Frame released");

    // Telemetry stats
    let stats = engine.decoder_stats();
    assert_eq!(stats.frames_queued, 2);
    assert_eq!(stats.frames_decoded, 1);
    assert_eq!(stats.frames_rendered, 1);

    // Progress updates
    engine
        .update_progress(10_000_000)
        .await
        .expect("Progress updated");
    assert_eq!(engine.position_ticks(), 10_000_000);

    // Pause and Resume
    engine.pause().await.expect("Paused");
    assert_eq!(engine.state(), PlaybackState::Paused);

    engine.resume().await.expect("Resumed");
    assert_eq!(engine.state(), PlaybackState::Playing);

    // Stop playback
    engine.stop().await.expect("Stopped");
    assert_eq!(engine.state(), PlaybackState::Stopped);
}
