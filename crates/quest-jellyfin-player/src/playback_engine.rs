//! Hardware video decoder pipeline, presentation clock synchronizer, and Jellyfin session coordination.

use crate::error::{PlayerError, Result};
use crate::format::{detect_item_format, DetectedStereoFormat};
use jellyfin_client::{
    BaseItem, JellyfinClient, MediaStreamType, PlayMethod, PlaybackProgressRequest,
    PlaybackStartRequest, PlaybackStopRequest,
};
use std::time::{Duration, Instant};
use url::Url;
use xr_video::audio::sync::AvSynchronizer;
use xr_video::config::VideoCodec;
use xr_video::decoder::{DecodedFrame, DecoderStats, HardwareVideoDecoder};

/// Playback execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackTransition {
    Start,
    Pause,
    Resume,
    Stop,
}

impl PlaybackState {
    fn transition(self, transition: PlaybackTransition) -> Option<Self> {
        match (self, transition) {
            (Self::Idle, PlaybackTransition::Start) => Some(Self::Playing),
            (Self::Playing, PlaybackTransition::Pause) => Some(Self::Paused),
            (Self::Paused, PlaybackTransition::Resume) => Some(Self::Playing),
            (Self::Playing | Self::Paused, PlaybackTransition::Stop) => Some(Self::Stopped),
            _ => None,
        }
    }
}

const PROGRESS_REPORT_INTERVAL: Duration = Duration::from_secs(10);

fn progress_report_due(state: PlaybackState, last_report: Option<Instant>, now: Instant) -> bool {
    state == PlaybackState::Playing
        && last_report
            .is_none_or(|last| now.saturating_duration_since(last) >= PROGRESS_REPORT_INTERVAL)
}

/// Coordinates hardware video decoding via `xr-video` and session reporting via `jellyfin-client`.
pub struct PlaybackEngine {
    client: JellyfinClient,
    decoder: Box<dyn HardwareVideoDecoder>,
    clock: AvSynchronizer,
    state: PlaybackState,
    current_item: Option<BaseItem>,
    play_session_id: Option<String>,
    media_source_id: Option<String>,
    last_progress_report: Option<Instant>,
    direct_play_url: Option<Url>,
    detected_format: Option<DetectedStereoFormat>,
    position_ticks: u64,
}

impl PlaybackEngine {
    /// Creates a new `PlaybackEngine` with a specific hardware or mock video decoder.
    pub fn new(client: JellyfinClient, decoder: Box<dyn HardwareVideoDecoder>) -> Self {
        Self {
            client,
            decoder,
            clock: AvSynchronizer::new(Default::default()),
            state: PlaybackState::Idle,
            current_item: None,
            play_session_id: None,
            media_source_id: None,
            last_progress_report: None,
            direct_play_url: None,
            detected_format: None,
            position_ticks: 0,
        }
    }

    /// Accessor for A/V presentation synchronizer.
    pub fn synchronizer(&mut self) -> &mut AvSynchronizer {
        &mut self.clock
    }

    /// Current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Current playback position in Jellyfin ticks (1 sec = 10,000,000 ticks).
    pub fn position_ticks(&self) -> u64 {
        self.position_ticks
    }

    /// Direct play URL loaded for the active item.
    pub fn direct_play_url(&self) -> Option<&Url> {
        self.direct_play_url.as_ref()
    }

    /// Detected 3D format for the active item.
    pub fn detected_format(&self) -> Option<&DetectedStereoFormat> {
        self.detected_format.as_ref()
    }

    /// Loads a media item, queries PlaybackInfo, detects 3D format, and configures the hardware decoder.
    pub async fn load_item(&mut self, item: BaseItem) -> Result<()> {
        if matches!(self.state, PlaybackState::Playing | PlaybackState::Paused) {
            return Err(PlayerError::Playback(
                "Stop the current session before loading another item".into(),
            ));
        }
        self.current_item = None;
        self.play_session_id = None;
        self.media_source_id = None;
        self.direct_play_url = None;
        self.detected_format = None;
        self.last_progress_report = None;
        self.position_ticks = 0;
        self.state = PlaybackState::Idle;
        let detected = detect_item_format(&item);
        let playback_info = self.client.playback_info(&item.id, None).await?;
        if let Some(error) = &playback_info.error_code {
            return Err(PlayerError::Playback(format!(
                "Jellyfin rejected playback: {error}"
            )));
        }
        let source = playback_info
            .media_sources
            .iter()
            .find(|source| source.supports_direct_play)
            .ok_or_else(|| {
                PlayerError::Playback(
                    "No direct-play media source; transcoding is not implemented".into(),
                )
            })?;
        let mut stream_url = self.client.direct_play_url(&item.id)?;
        stream_url
            .query_pairs_mut()
            .append_pair("MediaSourceId", &source.id);

        // Find primary video stream for decoder configuration
        let video_stream = source
            .media_streams
            .iter()
            .find(|s| s.stream_type == Some(MediaStreamType::Video))
            .or_else(|| {
                (playback_info.media_sources.len() == 1)
                    .then_some(&item.media_streams)?
                    .iter()
                    .find(|s| s.stream_type == Some(MediaStreamType::Video))
            });

        let width = video_stream
            .and_then(|s| s.width)
            .and_then(|w| u32::try_from(w).ok())
            .filter(|w| *w > 0)
            .ok_or_else(|| PlayerError::Playback("Missing video width".into()))?;
        let height = video_stream
            .and_then(|s| s.height)
            .and_then(|h| u32::try_from(h).ok())
            .filter(|h| *h > 0)
            .ok_or_else(|| PlayerError::Playback("Missing video height".into()))?;
        let codec = match video_stream
            .and_then(|s| s.codec.as_deref())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "av1" => VideoCodec::Av1,
            "hevc" | "h265" | "h.265" => VideoCodec::Hevc,
            codec => {
                return Err(PlayerError::Playback(format!(
                    "Unsupported or missing video codec: {codec}"
                )))
            }
        };

        // Initialize hardware decoder
        self.decoder.init(width, height, codec)?;

        self.current_item = Some(item);
        self.media_source_id = Some(source.id.clone());
        self.play_session_id = playback_info.play_session_id;
        self.last_progress_report = None;
        self.direct_play_url = Some(stream_url);
        self.detected_format = Some(detected);
        self.position_ticks = 0;
        self.state = PlaybackState::Idle;

        Ok(())
    }

    /// Starts playback, reporting to Jellyfin `/Sessions/Playing`.
    pub async fn play(&mut self) -> Result<()> {
        if self.state.transition(PlaybackTransition::Start).is_none() {
            return Err(PlayerError::Playback(
                "Playback can only start after loading an item".into(),
            ));
        }
        let item = self
            .current_item
            .as_ref()
            .ok_or_else(|| PlayerError::Playback("No item loaded".to_string()))?;

        let req = PlaybackStartRequest {
            item_id: item.id.clone(),
            play_session_id: self.play_session_id.clone(),
            play_method: PlayMethod::DirectPlay,
            can_seek: false,
            is_paused: false,
            position_ticks: self.position_ticks,
            media_source_id: self.media_source_id.clone(),
            audio_stream_index: None,
            subtitle_stream_index: None,
        };

        self.client.report_playing(&req).await?;
        self.state = self.state.transition(PlaybackTransition::Start).unwrap();
        self.last_progress_report = Some(Instant::now());
        Ok(())
    }

    /// Updates playback position and reports periodic progress at most once every 10 seconds.
    pub async fn update_progress(&mut self, current_ticks: u64) -> Result<()> {
        self.position_ticks = current_ticks;
        if progress_report_due(self.state, self.last_progress_report, Instant::now()) {
            if let Some(item) = &self.current_item {
                let mut req = PlaybackProgressRequest::time_update(
                    item.id.clone(),
                    self.play_session_id.clone(),
                    self.position_ticks,
                    false,
                );
                req.can_seek = false;
                req.media_source_id = self.media_source_id.clone();
                self.client.report_progress(&req).await?;
                self.last_progress_report = Some(Instant::now());
            }
        }
        Ok(())
    }

    /// Pauses playback and notifies Jellyfin session.
    pub async fn pause(&mut self) -> Result<()> {
        let Some(next_state) = self.state.transition(PlaybackTransition::Pause) else {
            return Ok(());
        };

        if let Some(item) = &self.current_item {
            let req = PlaybackProgressRequest {
                item_id: item.id.clone(),
                play_session_id: self.play_session_id.clone(),
                play_method: PlayMethod::DirectPlay,
                can_seek: false,
                is_paused: true,
                position_ticks: self.position_ticks,
                event_name: Some("Pause".to_string()),
                media_source_id: self.media_source_id.clone(),
            };
            self.client.report_progress(&req).await?;
        }
        self.state = next_state;
        Ok(())
    }

    /// Resumes playback from paused state.
    pub async fn resume(&mut self) -> Result<()> {
        let Some(next_state) = self.state.transition(PlaybackTransition::Resume) else {
            return Ok(());
        };

        if let Some(item) = &self.current_item {
            let req = PlaybackProgressRequest {
                item_id: item.id.clone(),
                play_session_id: self.play_session_id.clone(),
                play_method: PlayMethod::DirectPlay,
                can_seek: false,
                is_paused: false,
                position_ticks: self.position_ticks,
                event_name: Some("Unpause".to_string()),
                media_source_id: self.media_source_id.clone(),
            };
            self.client.report_progress(&req).await?;
        }
        self.state = next_state;
        Ok(())
    }

    /// Stops playback, notifies Jellyfin `/Sessions/Playing/Stopped`, and flushes the decoder.
    pub async fn stop(&mut self) -> Result<()> {
        let Some(next_state) = self.state.transition(PlaybackTransition::Stop) else {
            return Ok(());
        };
        if let Some(item) = &self.current_item {
            let mut req = PlaybackStopRequest::new(
                item.id.clone(),
                self.play_session_id.clone(),
                self.position_ticks,
            );
            req.media_source_id = self.media_source_id.clone();
            self.client.report_stopped(&req).await?;
        }

        self.decoder.flush()?;
        self.state = next_state;
        Ok(())
    }

    /// Queues an encoded video bitstream packet into the hardware decoder.
    pub fn queue_packet(&mut self, data: &[u8], pts_us: u64, is_keyframe: bool) -> Result<()> {
        self.decoder.queue_input_buffer(data, pts_us, is_keyframe)?;
        Ok(())
    }

    /// Dequeues a decoded frame from the hardware decoder.
    pub fn dequeue_frame(&mut self, timeout_us: i64) -> Result<Option<DecodedFrame>> {
        let frame = self.decoder.dequeue_output_buffer(timeout_us)?;
        Ok(frame)
    }

    /// Releases a frame back to the decoder or display surface.
    pub fn release_frame(&mut self, buffer_index: usize, render: bool) -> Result<()> {
        self.decoder.release_output_buffer(buffer_index, render)?;
        Ok(())
    }

    /// Returns telemetry statistics from the hardware decoder.
    pub fn decoder_stats(&self) -> DecoderStats {
        self.decoder.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_state_transitions_follow_session_lifecycle() {
        assert_eq!(
            PlaybackState::Idle.transition(PlaybackTransition::Start),
            Some(PlaybackState::Playing)
        );
        assert_eq!(
            PlaybackState::Playing.transition(PlaybackTransition::Pause),
            Some(PlaybackState::Paused)
        );
        assert_eq!(
            PlaybackState::Paused.transition(PlaybackTransition::Resume),
            Some(PlaybackState::Playing)
        );
        assert_eq!(
            PlaybackState::Playing.transition(PlaybackTransition::Stop),
            Some(PlaybackState::Stopped)
        );
        assert_eq!(
            PlaybackState::Paused.transition(PlaybackTransition::Stop),
            Some(PlaybackState::Stopped)
        );
    }

    #[test]
    fn playback_state_transitions_reject_invalid_commands() {
        for state in [PlaybackState::Idle, PlaybackState::Stopped] {
            assert_eq!(state.transition(PlaybackTransition::Pause), None);
            assert_eq!(state.transition(PlaybackTransition::Resume), None);
            assert_eq!(state.transition(PlaybackTransition::Stop), None);
        }
        assert_eq!(
            PlaybackState::Playing.transition(PlaybackTransition::Start),
            None
        );
        assert_eq!(
            PlaybackState::Paused.transition(PlaybackTransition::Start),
            None
        );
        assert_eq!(
            PlaybackState::Stopped.transition(PlaybackTransition::Start),
            None
        );
    }

    #[test]
    fn progress_reports_are_limited_to_playing_sessions_and_ten_second_intervals() {
        let now = Instant::now();
        let last = now - PROGRESS_REPORT_INTERVAL;

        assert!(progress_report_due(PlaybackState::Playing, None, now));
        assert!(!progress_report_due(
            PlaybackState::Playing,
            Some(now - Duration::from_millis(9_999)),
            now
        ));
        assert!(progress_report_due(PlaybackState::Playing, Some(last), now));
        assert!(!progress_report_due(PlaybackState::Paused, None, now));
        assert!(!progress_report_due(PlaybackState::Idle, Some(last), now));
    }
}
