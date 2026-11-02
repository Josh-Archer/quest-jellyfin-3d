//! Quest 3 playback capability adjustments layered over `jellyfin-client`.
//!
//! The registry crate owns the shared Jellyfin models. This app keeps the
//! Quest-specific policy override here so its published profile remains
//! reusable by other clients.

use jellyfin_client::{
    evaluate_direct_play as evaluate_base, quest_device_profile as base_profile, BaseItem,
    DeviceProfile, DirectPlayDecision, MediaStreamType,
};

/// Returns the Quest 3 profile with this app's 8K30 and 4K60 decode limits.
pub fn quest_device_profile() -> DeviceProfile {
    let mut profile = base_profile();
    for codec_profile in &mut profile.codec_profiles {
        if codec_profile.codec.eq_ignore_ascii_case("hevc")
            || codec_profile.codec.eq_ignore_ascii_case("av1")
        {
            set_limit(&mut codec_profile.conditions, "VideoFramerate", "30");
        } else if codec_profile.codec.eq_ignore_ascii_case("h264") {
            set_limit(&mut codec_profile.conditions, "VideoFramerate", "60");
        }
    }
    profile
}

fn set_limit(conditions: &mut [jellyfin_client::ProfileCondition], property: &str, value: &str) {
    if let Some(condition) = conditions
        .iter_mut()
        .find(|condition| condition.property == property)
    {
        condition.value = value.to_owned();
    }
}

/// Applies the app's stricter frame-rate caps on top of the shared evaluator.
pub fn evaluate_direct_play(profile: &DeviceProfile, item: &BaseItem) -> DirectPlayDecision {
    let base = evaluate_base(profile, item);
    if base != DirectPlayDecision::DirectPlay {
        return base;
    }

    let Some(stream) = item
        .media_streams
        .iter()
        .find(|stream| stream.stream_type == Some(MediaStreamType::Video))
    else {
        return DirectPlayDecision::DirectPlay;
    };
    let codec = stream
        .codec
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let width = stream.width.unwrap_or(0);
    let height = stream.height.unwrap_or(0);
    let framerate = stream
        .real_frame_rate
        .or(stream.average_frame_rate)
        .unwrap_or(0.0);

    if (codec.contains("hevc") || codec.contains("h265") || codec == "av1")
        && (width > 4096 || height > 2160)
        && framerate > 30.0
    {
        return DirectPlayDecision::TranscodeRequired {
            reason: format!(
                "Framerate {:.1}fps at {}x{} exceeds Quest 3 8K30 hardware cap",
                framerate, width, height
            ),
        };
    }
    if (codec.contains("h264") || codec.contains("avc")) && framerate > 60.0 {
        return DirectPlayDecision::TranscodeRequired {
            reason: format!(
                "Framerate {:.1}fps exceeds Quest 3 H.264 60fps hardware cap",
                framerate
            ),
        };
    }
    DirectPlayDecision::DirectPlay
}
