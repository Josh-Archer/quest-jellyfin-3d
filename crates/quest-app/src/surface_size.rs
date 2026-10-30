//! Track actual decoded dimensions before changing an OpenXR surface.
use quest_jellyfin_player::{build_stereo_eye_layer_specs, EyeOrder, StereoFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl SurfaceSize {
    pub fn new(
        width: u32,
        height: u32,
        format: StereoFormat,
        order: EyeOrder,
    ) -> Result<Self, String> {
        // Mono's eye-layer helper does not check dimensions; validate them here too.
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err("Invalid decoded video dimensions".into());
        }
        build_stereo_eye_layer_specs(format, order, width, height).map_err(|e| e.to_string())?;
        Ok(Self { width, height })
    }
    pub fn requires_resize(self, width: u32, height: u32) -> bool {
        self.width != width || self.height != height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_changed_even_stereo_frames_and_rejects_incomplete_eyes() {
        let size = SurfaceSize::new(
            1920,
            1080,
            StereoFormat::HalfSideBySide,
            EyeOrder::LeftRight,
        )
        .unwrap();
        assert!(!size.requires_resize(1920, 1080));
        assert!(size.requires_resize(1280, 720));
        assert!(
            SurfaceSize::new(1279, 720, StereoFormat::HalfSideBySide, EyeOrder::LeftRight).is_err()
        );
        assert!(
            SurfaceSize::new(1280, 719, StereoFormat::FullOverUnder, EyeOrder::TopBottom).is_err()
        );
    }
    #[test]
    fn rejects_empty_and_overflowing_mono_frames() {
        for (w, h) in [(0, 720), (1280, 0), (u32::MAX, 10)] {
            assert!(SurfaceSize::new(w, h, StereoFormat::Mono, EyeOrder::LeftRight).is_err());
        }
    }
}
