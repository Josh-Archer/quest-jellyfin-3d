//! Hardware video decoder pipeline, presentation clock synchronizer, and Jellyfin session coordination.

use crate::error::{PlayerError, Result};
use crate::format::{detect_item_format, DetectedStereoFormat};
use jellyfin_client::{
    BaseItem, JellyfinClient, MediaStreamType, PlayMethod, PlaybackProgressRequest,
    PlaybackStartRequest, PlaybackStopRequest,
};
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

/// Coordinates hardware video decoding via `xr-video` and session reporting via `jellyfin-client`.
pub struct PlaybackEngine {
    client: JellyfinClient,
    decoder: Box<dyn HardwareVideoDecoder>,
    clock: AvSynchronizer,
    state: PlaybackState,
    current_item: Option<BaseItem>,
    play_session_id: Option<String>,
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
        let detected = detect_item_format(&item);
        let playback_info = self.client.playback_info(&item.id, None).await?;
        let stream_url = self.client.direct_play_url(&item.id)?;

        // Find primary video stream for decoder configuration
        let video_stream = item
            .media_streams
            .iter()
            .find(|s| s.stream_type == Some(MediaStreamType::Video));

        let width = video_stream
            .and_then(|s| s.width)
            .and_then(|w| u32::try_from(w).ok())
            .unwrap_or(1920);
        let height = video_stream
            .and_then(|s| s.height)
            .and_then(|h| u32::try_from(h).ok())
            .unwrap_or(1080);
        let codec = match video_stream
            .and_then(|s| s.codec.as_deref())
            .unwrap_or("hevc")
            .to_lowercase()
            .as_str()
        {
            "av1" => VideoCodec::Av1,
            _ => VideoCodec::Hevc,
        };

        // Initialize hardware decoder
        self.decoder.init(width, height, codec)?;

        self.current_item = Some(item);
        self.play_session_id = playback_info.play_session_id;
        self.direct_play_url = Some(stream_url);
        self.detected_format = Some(detected);
        self.position_ticks = 0;
        self.state = PlaybackState::Idle;

        Ok(())
    }

    /// Starts playback, reporting to Jellyfin `/Sessions/Playing`.
    pub async fn play(&mut self) -> Result<()> {
        let item = self
            .current_item
            .as_ref()
            .ok_or_else(|| PlayerError::Playback("No item loaded".to_string()))?;

        let req = PlaybackStartRequest {
            item_id: item.id.clone(),
            play_session_id: self.play_session_id.clone(),
            play_method: PlayMethod::DirectPlay,
            can_seek: true,
            is_paused: false,
            position_ticks: self.position_ticks,
            media_source_id: None,
            audio_stream_index: None,
            subtitle_stream_index: None,
        };

        self.client.report_playing(&req).await?;
        self.state = PlaybackState::Playing;
        Ok(())
    }

    /// Updates playback progress ticks and reports to Jellyfin `/Sessions/Playing/Progress`.
    pub async fn update_progress(&mut self, current_ticks: u64) -> Result<()> {
        self.position_ticks = current_ticks;
        if self.state == PlaybackState::Playing {
            if let Some(item) = &self.current_item {
                let req = PlaybackProgressRequest::time_update(
                    item.id.clone(),
                    self.play_session_id.clone(),
                    self.position_ticks,
                    false,
                );
                self.client.report_progress(&req).await?;
            }
        }
        Ok(())
    }

    /// Pauses playback and notifies Jellyfin session.
    pub async fn pause(&mut self) -> Result<()> {
        if self.state != PlaybackState::Playing {
            return Ok(());
        }

        if let Some(item) = &self.current_item {
            let req = PlaybackProgressRequest {
                item_id: item.id.clone(),
                play_session_id: self.play_session_id.clone(),
                play_method: PlayMethod::DirectPlay,
                can_seek: true,
                is_paused: true,
                position_ticks: self.position_ticks,
                event_name: Some("Pause".to_string()),
                media_source_id: None,
            };
            self.client.report_progress(&req).await?;
        }
        self.state = PlaybackState::Paused;
        Ok(())
    }

    /// Resumes playback from paused state.
    pub async fn resume(&mut self) -> Result<()> {
        if self.state != PlaybackState::Paused {
            return Ok(());
        }

        if let Some(item) = &self.current_item {
            let req = PlaybackProgressRequest {
                item_id: item.id.clone(),
                play_session_id: self.play_session_id.clone(),
                play_method: PlayMethod::DirectPlay,
                can_seek: true,
                is_paused: false,
                position_ticks: self.position_ticks,
                event_name: Some("Unpause".to_string()),
                media_source_id: None,
            };
            self.client.report_progress(&req).await?;
        }
        self.state = PlaybackState::Playing;
        Ok(())
    }

    /// Stops playback, notifies Jellyfin `/Sessions/Playing/Stopped`, and flushes the decoder.
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(item) = &self.current_item {
            let req = PlaybackStopRequest::new(
                item.id.clone(),
                self.play_session_id.clone(),
                self.position_ticks,
            );
            self.client.report_stopped(&req).await?;
        }

        self.decoder.flush()?;
        self.state = PlaybackState::Stopped;
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
