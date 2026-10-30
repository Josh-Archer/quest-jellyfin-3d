//! Edge-triggered controller shortcuts for immersive playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    TogglePlayback,
    SeekBack,
    SeekForward,
    NextAudioTrack,
}

#[derive(Debug, Default)]
pub struct PlaybackControls {
    trigger_down: bool,
    grip_down: bool,
    stick_direction: i8,
}
impl PlaybackControls {
    /// Trigger toggles pause; a horizontal stick deflection seeks once until centered.
    pub fn update(&mut self, trigger: bool, horizontal: f32, grip: bool) -> Option<Command> {
        let direction = if horizontal.is_finite() && horizontal >= 0.75 {
            1
        } else if horizontal.is_finite() && horizontal <= -0.75 {
            -1
        } else if !horizontal.is_finite() || horizontal.abs() <= 0.35 {
            0
        } else {
            self.stick_direction
        };
        let toggle = trigger && !self.trigger_down;
        let next_audio = grip && !self.grip_down;
        self.grip_down = grip;
        let seek = self.stick_direction == 0 && direction != 0;
        self.trigger_down = trigger;
        self.stick_direction = direction;
        if toggle {
            Some(Command::TogglePlayback)
        } else if next_audio {
            Some(Command::NextAudioTrack)
        } else if seek && direction < 0 {
            Some(Command::SeekBack)
        } else if seek {
            Some(Command::SeekForward)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trigger_fires_once_per_press_and_rearms_on_release() {
        let mut c = PlaybackControls::default();
        assert_eq!(c.update(true, 0.0, false), Some(Command::TogglePlayback));
        assert_eq!(c.update(true, 0.0, false), None);
        assert_eq!(c.update(false, 0.0, false), None);
        assert_eq!(c.update(true, 0.0, false), Some(Command::TogglePlayback));
    }
    #[test]
    fn grip_cycles_audio_only_on_new_presses() {
        let mut c = PlaybackControls::default();
        assert_eq!(c.update(false, 0.0, true), Some(Command::NextAudioTrack));
        assert_eq!(c.update(false, 0.0, true), None);
        assert_eq!(c.update(false, 0.0, false), None);
        assert_eq!(c.update(false, 0.0, true), Some(Command::NextAudioTrack));
    }
    #[test]
    fn stick_hysteresis_prevents_repeats_and_requires_centering() {
        let mut c = PlaybackControls::default();
        assert_eq!(c.update(false, -0.8, false), Some(Command::SeekBack));
        assert_eq!(c.update(false, -0.6, false), None);
        assert_eq!(c.update(false, 0.9, false), None);
        assert_eq!(c.update(false, 0.0, false), None);
        assert_eq!(c.update(false, 0.8, false), Some(Command::SeekForward));
        assert_eq!(c.update(false, f32::NAN, false), None);
        assert_eq!(c.update(false, -0.8, false), Some(Command::SeekBack));
    }
}
