use jellyfin_client::{DeviceProfile, DirectPlayDecision};
use quest_app::playback_profile::{evaluate_direct_play, quest_device_profile};
use serde_json::{json, Value};

fn item(codec: &str, width: i32, height: i32, fps: f64) -> jellyfin_client::BaseItem {
    serde_json::from_value(json!({
        "Id": "cap-test",
        "Name": "Cap test",
        "Container": "mkv",
        "MediaStreams": [{
            "Type": "Video",
            "Codec": codec,
            "Width": width,
            "Height": height,
            "RealFrameRate": fps
        }]
    }))
    .unwrap()
}

fn profile_json() -> Value {
    serde_json::to_value(quest_device_profile()).unwrap()
}

#[test]
fn quest_device_profile_advertises_jellyfin_codec_limits() {
    let profile = profile_json();
    assert_eq!(profile["Name"], "Meta Quest 3");
    assert_eq!(profile["DirectPlayProfiles"][0]["Type"], "Video");
    assert_eq!(
        profile["DirectPlayProfiles"][0]["VideoCodec"],
        "hevc,h265,av1,avc,h264,vp9"
    );
    for codec in ["hevc", "av1"] {
        let limits = profile["CodecProfiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["Codec"] == codec)
            .unwrap();
        assert_eq!(limits["Conditions"][0]["Value"], "8192");
        assert_eq!(limits["Conditions"][1]["Value"], "4320");
        assert_eq!(limits["Conditions"][2]["Property"], "VideoFramerate");
        assert_eq!(limits["Conditions"][2]["Value"], "30");
    }
    let h264 = profile["CodecProfiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["Codec"] == "h264")
        .unwrap();
    assert_eq!(h264["Conditions"][0]["Value"], "4096");
    assert_eq!(h264["Conditions"][1]["Value"], "4096");
    assert_eq!(h264["Conditions"][2]["Value"], "60");
    let _: DeviceProfile = serde_json::from_value(profile).unwrap();
}

#[test]
fn codec_frame_rate_caps_transcode_above_8k30_and_4k60() {
    let profile = quest_device_profile();
    for codec in ["hevc", "av1"] {
        match evaluate_direct_play(&profile, &item(codec, 7680, 4320, 30.1)) {
            DirectPlayDecision::TranscodeRequired { reason } => {
                assert!(reason.contains("8K30 hardware cap"), "{reason}");
            }
            other => panic!("Expected 8K30 transcode decision, got {other:?}"),
        }
        assert_eq!(
            evaluate_direct_play(&profile, &item(codec, 7680, 4320, 30.0)),
            DirectPlayDecision::DirectPlay
        );
    }
    match evaluate_direct_play(&profile, &item("h264", 1920, 1080, 60.1)) {
        DirectPlayDecision::TranscodeRequired { reason } => {
            assert!(reason.contains("H.264 60fps hardware cap"), "{reason}");
        }
        other => panic!("Expected H.264 transcode decision, got {other:?}"),
    }
    assert_eq!(
        evaluate_direct_play(&profile, &item("h264", 1920, 1080, 60.0)),
        DirectPlayDecision::DirectPlay
    );
}
