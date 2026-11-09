//! Shared DeskQuest manipulation, with transport hit targets kept separate from screen grabs.
use xr_core::{
    spatial::{
        GrabStateMachine, InteractionEvent, InteractionInput, InteractionSource, ScreenPose,
        ScreenTransform, Vec3,
    },
    DisplayMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Seek(u16),
    PlayPause,
    Back,
    Forward,
    Audio,
    Subtitles,
    Exit,
    Smaller,
    Larger,
    Flatter,
    Curvier,
    Recenter,
    Hide,
    Screen,
    SwapEyes,
    ComfortMono,
    AlignMinus,
    AlignPlus,
    ResetAlignment,
}

pub const ARTWORK_WIDTH: i32 = 768;
pub const MAIN_HEIGHT: i32 = 200;
pub const TOOLS_HEIGHT: i32 = 392;
pub fn artwork_height(tools: bool) -> i32 {
    if tools {
        TOOLS_HEIGHT
    } else {
        MAIN_HEIGHT
    }
}

/// Matches PlayerArtwork's embedded transport strip. Pixel gaps remain inert.
pub fn button_at((u, v): (f32, f32), tools: bool) -> Option<Button> {
    if !u.is_finite() || !v.is_finite() || !(0.0..1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }
    let x = u * ARTWORK_WIDTH as f32;
    let mut y = v * artwork_height(tools) as f32;
    if tools && (42.0..102.0).contains(&y) {
        for (i, button) in [
            Button::Smaller,
            Button::Larger,
            Button::Flatter,
            Button::Curvier,
            Button::Recenter,
        ]
        .into_iter()
        .enumerate()
        {
            let left = 24.0 + i as f32 * 146.4;
            if (left..left + 134.4).contains(&x) {
                return Some(button);
            }
        }
    }
    if tools && (116.0..176.0).contains(&y) {
        if (24.0..224.0).contains(&x) {
            return Some(Button::ComfortMono);
        }
        if (240.0..440.0).contains(&x) {
            return Some(Button::SwapEyes);
        }
        for (left, right, button) in [
            (464.0, 544.0, Button::AlignMinus),
            (556.0, 636.0, Button::AlignPlus),
            (648.0, 744.0, Button::ResetAlignment),
        ] {
            if (left..right).contains(&x) {
                return Some(button);
            }
        }
    }
    if tools {
        y -= 192.0;
    }
    if (64.0..112.0).contains(&y) && (24.0..=744.0).contains(&x) {
        return Some(Button::Seek(((x - 24.0) / 720.0 * 1000.0).round() as u16));
    }
    if (8.0..68.0).contains(&y) && (696.0..756.0).contains(&x) {
        return Some(Button::Exit);
    }
    if (120.0..184.0).contains(&y) {
        return [
            (24.0, 88.0, Button::PlayPause),
            (100.0, 160.0, Button::Back),
            (168.0, 228.0, Button::Forward),
            (364.0, 460.0, Button::Audio),
            (472.0, 568.0, Button::Subtitles),
            (580.0, 676.0, Button::Screen),
            (696.0, 756.0, Button::Hide),
        ]
        .into_iter()
        .find(|(left, right, _)| (*left..*right).contains(&x))
        .map(|(_, _, b)| b);
    }
    None
}
impl Button {
    /// Stable artwork highlight ID, shared with PlayerArtwork.
    pub fn artwork_id(self) -> i32 {
        match self {
            Self::PlayPause => 0,
            Self::Back => 1,
            Self::Forward => 2,
            Self::Audio => 3,
            Self::Exit => 4,
            Self::Screen => 5,
            Self::Hide => 6,
            Self::Smaller => 7,
            Self::Larger => 8,
            Self::Flatter => 9,
            Self::Curvier => 10,
            Self::Recenter => 11,
            Self::Seek(_) => 12,
            Self::SwapEyes => 13,
            Self::ComfortMono => 14,
            Self::AlignMinus => 15,
            Self::AlignPlus => 16,
            Self::ResetAlignment => 17,
            Self::Subtitles => 18,
        }
    }
}

/// A sub-surface inside the movie's lower edge. Curved movies get a matching cylinder
/// segment with the same axis, 4 mm towards the viewer, so drawing and ray hits agree.
pub fn panel_transform(screen: &ScreenTransform, tools: bool) -> ScreenTransform {
    let (width, height) = screen.size_m();
    let panel_width = (width * 0.9).min(2.8);
    let panel_height =
        (panel_width * artwork_height(tools) as f32 / ARTWORK_WIDTH as f32).min(height * 0.84);
    let centre_v = 0.94 - panel_height / (2.0 * height);
    let at = screen.surface_pose(0.5, centre_v);
    let mut panel = screen.clone();
    panel.position = at.position + at.orientation.rotate_vec(Vec3::new(0.0, 0.0, 0.004));
    panel.quad_size = (panel_width, panel_height);
    if screen.display_mode == DisplayMode::CurvedCylinder {
        panel.curve_radius = screen.curve_radius - 0.004;
        panel.curve_angle = screen.curve_angle * (panel_width / width);
        panel.aspect_ratio = panel.curve_radius * panel.curve_angle / panel_height;
    }
    panel
}

/// Readable cue strip just above the main transport, tracking screen distance/curve.
/// Placement ignores the tools drawer so cues do not jump when Screen opens.
pub fn subtitle_transform(screen: &ScreenTransform) -> ScreenTransform {
    let (width, height) = screen.size_m();
    let transport_width = (width * 0.9).min(2.8);
    let transport_height =
        (transport_width * MAIN_HEIGHT as f32 / ARTWORK_WIDTH as f32).min(height * 0.84);
    let sub_width = (width * 0.72).min(2.4).max(0.8);
    let sub_height = (sub_width * 0.16).min(height * 0.14).max(0.08);
    let transport_top_v = 0.94 - transport_height / height;
    let gap_v = (0.02 / height).min(transport_top_v * 0.25);
    let centre_v =
        (transport_top_v - gap_v - sub_height / (2.0 * height)).clamp(0.2, 0.92);
    let at = screen.surface_pose(0.5, centre_v);
    let mut panel = screen.clone();
    panel.position = at.position + at.orientation.rotate_vec(Vec3::new(0.0, 0.0, 0.004));
    panel.quad_size = (sub_width, sub_height);
    if screen.display_mode == DisplayMode::CurvedCylinder {
        panel.curve_radius = screen.curve_radius - 0.004;
        panel.curve_angle = screen.curve_angle * (sub_width / width);
        panel.aspect_ratio = panel.curve_radius * panel.curve_angle / sub_height;
    }
    panel
}

pub fn resize(screen: &mut ScreenTransform, factor: f32) {
    let base = screen.clone();
    screen.set_uniform_scale_from(&base, factor);
}

/// Preserve arc length, height, and screen centre when switching between flat and curved.
pub fn bend(screen: &mut ScreenTransform, delta: f32) {
    let (width, height) = screen.size_m();
    let angle = if screen.display_mode == DisplayMode::FlatQuad {
        0.0
    } else {
        screen.curve_angle
    };
    let minimum = (width / 5.0).max(0.2);
    let next = if angle == 0.0 && delta > 0.0 {
        delta.max(minimum)
    } else {
        (angle + delta).clamp(0.0, 2.4)
    };
    screen.quad_size = (width, height);
    if next < minimum - 1e-5 {
        screen.display_mode = DisplayMode::FlatQuad;
    } else {
        screen.display_mode = DisplayMode::CurvedCylinder;
        screen.curve_angle = next;
        screen.curve_radius = width / next;
        screen.aspect_ratio = width / height;
    }
}

pub struct ScreenControls {
    pub grab: GrabStateMachine,
    pub visible: bool,
    pub tools_open: bool,
    pub stereo_alignment_supported: bool,
    pub hovered: Option<Button>,
    pub opacity: f32,
    idle_seconds: f32,
    playing: bool,
    pointing_at_controls: bool,
    pressed: [bool; 4],
    alignment_hold: [Option<(Button, f32)>; 4],
}
impl Default for ScreenControls {
    fn default() -> Self {
        Self {
            grab: GrabStateMachine::new(),
            visible: true,
            tools_open: false,
            stereo_alignment_supported: false,
            hovered: None,
            opacity: 1.0,
            idle_seconds: 0.0,
            playing: false,
            pointing_at_controls: false,
            pressed: [false; 4],
            alignment_hold: [None; 4],
        }
    }
}
fn source_index(source: InteractionSource) -> usize {
    use xr_core::spatial::Handedness;
    match source {
        InteractionSource::Controller(Handedness::Left) => 0,
        InteractionSource::Controller(Handedness::Right) => 1,
        InteractionSource::Hand(Handedness::Left) => 2,
        InteractionSource::Hand(Handedness::Right) => 3,
    }
}
impl ScreenControls {
    /// Losing focus/tracking releases the grab and disarms held inputs until release.
    pub fn suspend(&mut self) {
        self.grab.release();
        self.pressed = [true; 4];
        self.alignment_hold = [None; 4];
        self.visible = true;
        self.idle_seconds = 0.0;
        self.hovered = None;
    }
    pub fn advance(&mut self, playing: bool, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        if self.playing && !playing {
            self.visible = true;
            self.idle_seconds = 0.0;
        }
        self.playing = playing;
        if self.pointing_at_controls || self.grab.is_grabbing() || self.tools_open {
            self.idle_seconds = 0.0;
        } else if playing {
            self.idle_seconds += dt;
            if self.idle_seconds >= 6.0 {
                self.visible = false;
            }
        }
        let target = if self.visible { 1.0 } else { 0.0 };
        self.opacity += (target - self.opacity).clamp(-dt * 5.0, dt * 5.0);
    }
    pub fn update(
        &mut self,
        inputs: &[InteractionInput],
        head: ScreenPose,
        screen: &mut ScreenTransform,
        dt: f32,
    ) -> Option<Button> {
        self.grab.set_viewer(head.position);
        let mut action = None;
        self.hovered = None;
        self.pointing_at_controls = false;
        let panel = panel_transform(screen, self.tools_open);
        let mut candidates = Vec::new();
        let mut active = self.grab.grab_session().map(|s| s.source);
        if active.is_some_and(|owner| !inputs.iter().any(|i| i.source == owner)) {
            self.grab.release();
            active = None;
        }
        for index in 0..4 {
            if !inputs
                .iter()
                .any(|input| source_index(input.source) == index)
            {
                self.alignment_hold[index] = None;
            }
        }
        for input in inputs {
            let index = source_index(input.source);
            let click = input.is_selecting && !self.pressed[index];
            self.pressed[index] = input.is_selecting;
            if !input.is_selecting {
                self.alignment_hold[index] = None;
            }
            let panel_hit = input
                .aim_ray
                .as_ref()
                .and_then(|ray| self.visible.then(|| panel.intersect_ray(ray)).flatten());
            let mut candidate = *input;
            if panel_hit.is_some() && active.is_none() {
                self.pointing_at_controls = true;
                let target = panel_hit
                    .and_then(|hit| button_at(hit.uv, self.tools_open))
                    .filter(|button| {
                        self.stereo_alignment_supported
                            || !matches!(
                                button,
                                Button::AlignMinus | Button::AlignPlus | Button::ResetAlignment
                            )
                    });
                if self.hovered.is_none() || click {
                    self.hovered = target;
                }
                candidate.is_grabbing = false;
                candidate.aim_ray = None;
                let mut repeat = false;
                if click {
                    self.alignment_hold[index] = target
                        .filter(|button| matches!(button, Button::AlignMinus | Button::AlignPlus))
                        .map(|button| (button, 0.45));
                } else if input.is_selecting {
                    if let Some((button, remaining)) = &mut self.alignment_hold[index] {
                        if Some(*button) == target {
                            *remaining -= if dt.is_finite() {
                                dt.clamp(0.0, 0.25)
                            } else {
                                0.0
                            };
                            if *remaining <= 0.0 {
                                repeat = true;
                                *remaining += 0.15;
                            }
                        } else {
                            self.alignment_hold[index] = None;
                        }
                    }
                }
                if (click || repeat) && action.is_none() {
                    action = target;
                }
            } else {
                self.alignment_hold[index] = None;
                let hit = input
                    .aim_ray
                    .as_ref()
                    .and_then(|ray| ScreenTransform::find_closest_window_hit(ray, [(0, &*screen)]));
                // Hands pinch only the outer handle region; pinching the movie reveals controls.
                if matches!(input.source, InteractionSource::Hand(_)) {
                    let handle = hit.is_some_and(|(_, h)| {
                        h.uv.0 < 0.12 || h.uv.0 > 0.88 || h.uv.1 < 0.12 || h.uv.1 > 0.88
                    });
                    candidate.is_grabbing =
                        input.is_selecting && (handle || active == Some(input.source));
                }
                if click && hit.is_some() && !candidate.is_grabbing && active.is_none() {
                    self.visible = true;
                    self.idle_seconds = 0.0;
                }
            }
            candidates.push(candidate);
        }
        // Stable owner while dragging; otherwise give a newly squeezed hand priority.
        let primary = candidates
            .iter()
            .find(|i| Some(i.source) == active)
            .or_else(|| candidates.iter().find(|i| i.is_grabbing))
            .or_else(|| {
                candidates.iter().find(|i| {
                    i.aim_ray.as_ref().is_some_and(|ray| {
                        ScreenTransform::find_closest_window_hit(ray, [(0, &*screen)]).is_some()
                    })
                })
            })
            .or_else(|| candidates.first());
        if let Some(input) = primary {
            let other = candidates.iter().find(|i| i.source != input.source);
            if let Some(event) = self.grab.update_two(input, other, [(0, &*screen)], dt) {
                match event {
                    InteractionEvent::GrabUpdated { transform, .. } => *screen = transform,
                    InteractionEvent::GrabEnded {
                        final_transform, ..
                    } => *screen = final_transform,
                    _ => {}
                }
            }
        } else {
            self.grab.release();
        }
        if action.is_some() {
            self.idle_seconds = 0.0;
        }
        if action == Some(Button::Screen) {
            self.tools_open = !self.tools_open;
        }
        if action == Some(Button::Hide) {
            self.visible = false;
            self.tools_open = false;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xr_core::spatial::{Handedness, Quat, Ray};
    fn screen() -> ScreenTransform {
        ScreenTransform {
            position: Vec3::new(0.0, 1.5, -2.2),
            orientation: Quat::IDENTITY,
            quad_size: (4.0, 2.25),
            display_mode: DisplayMode::FlatQuad,
            ..Default::default()
        }
    }
    fn head() -> ScreenPose {
        ScreenPose::new(Vec3::new(0.0, 1.5, 0.0), Quat::IDENTITY)
    }
    fn input(at: Vec3, select: bool) -> InteractionInput {
        InteractionInput {
            source: InteractionSource::Controller(Handedness::Right),
            aim_ray: Some(Ray::new(head().position, at - head().position)),
            grab_pose: Some(head()),
            is_grabbing: false,
            is_selecting: select,
            thumbstick: (0.0, 0.0),
        }
    }
    #[test]
    fn alignment_is_opt_in_and_hold_repeat_requires_a_fresh_press_after_focus_loss() {
        let mut controls = ScreenControls {
            tools_open: true,
            ..Default::default()
        };
        let mut screen = screen();
        let at = panel_transform(&screen, true)
            .surface_pose(596.0 / 768.0, 146.0 / 392.0)
            .position;
        let released = input(at, false);
        let held = input(at, true);
        controls.update(&[released], head(), &mut screen, 0.01);
        assert_eq!(controls.update(&[held], head(), &mut screen, 0.01), None);
        controls.stereo_alignment_supported = true;
        controls.update(&[released], head(), &mut screen, 0.01);
        assert_eq!(
            controls.update(&[held], head(), &mut screen, 0.01),
            Some(Button::AlignPlus)
        );
        for _ in 0..4 {
            assert_eq!(controls.update(&[held], head(), &mut screen, 0.1), None);
        }
        assert_eq!(
            controls.update(&[held], head(), &mut screen, 0.1),
            Some(Button::AlignPlus)
        );
        controls.update(&[], head(), &mut screen, 0.1);
        for _ in 0..10 {
            assert_eq!(controls.update(&[held], head(), &mut screen, 0.1), None);
        }
        controls.update(&[released], head(), &mut screen, 0.01);
        assert_eq!(
            controls.update(&[held], head(), &mut screen, 0.01),
            Some(Button::AlignPlus)
        );
        controls.suspend();
        for _ in 0..10 {
            assert_eq!(controls.update(&[held], head(), &mut screen, 0.1), None);
        }
        controls.update(&[released], head(), &mut screen, 0.01);
        assert_eq!(
            controls.update(&[held], head(), &mut screen, 0.01),
            Some(Button::AlignPlus)
        );
        for (x, expected) in [
            (504.0, Button::AlignMinus),
            (596.0, Button::AlignPlus),
            (696.0, Button::ResetAlignment),
        ] {
            assert_eq!(button_at((x / 768.0, 146.0 / 392.0), true), Some(expected));
        }
        for x in [550.0, 642.0] {
            assert_eq!(button_at((x / 768.0, 146.0 / 392.0), true), None);
        }
    }
    #[test]
    fn every_artwork_button_has_one_target_and_gaps_are_inert() {
        for tools in [false, true] {
            let h = artwork_height(tools) as f32;
            for (x, button) in [
                (56.0, Button::PlayPause),
                (130.0, Button::Back),
                (198.0, Button::Forward),
                (412.0, Button::Audio),
                (520.0, Button::Subtitles),
                (628.0, Button::Screen),
                (726.0, Button::Hide),
            ] {
                assert_eq!(
                    button_at(
                        (x / 768.0, (152.0 + if tools { 192.0 } else { 0.0 }) / h),
                        tools
                    ),
                    Some(button)
                );
            }
            assert_eq!(
                button_at(
                    (726.0 / 768.0, (38.0 + if tools { 192.0 } else { 0.0 }) / h),
                    tools
                ),
                Some(Button::Exit)
            );
            for uv in [
                (0.0, 0.5),
                (f32::NAN, 0.4),
                (1.0, 0.4),
                (0.5, f32::INFINITY),
                (466.0 / 768.0, (152.0 + if tools { 192.0 } else { 0.0 }) / h),
            ] {
                assert_eq!(button_at(uv, tools), None);
            }
        }
        for (column, button) in [
            Button::Smaller,
            Button::Larger,
            Button::Flatter,
            Button::Curvier,
            Button::Recenter,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                button_at(((91.2 + column as f32 * 146.4) / 768.0, 72.0 / 392.0), true),
                Some(button)
            );
        }
    }
    #[test]
    fn subtitle_strip_stays_above_transport_and_tracks_the_movie() {
        for curved in [false, true] {
            let mut movie = screen();
            if curved {
                bend(&mut movie, 1.2);
            }
            let transport = panel_transform(&movie, false);
            let tools = panel_transform(&movie, true);
            let cue = subtitle_transform(&movie);
            let local = movie
                .orientation
                .conjugate()
                .rotate_vec(cue.position - movie.position);
            let transport_local = movie
                .orientation
                .conjugate()
                .rotate_vec(transport.position - movie.position);
            assert!(local.y > transport_local.y);
            assert!((local.z - 0.004).abs() < 1e-5);
            assert!(cue.size_m().0 <= movie.size_m().0 + 1e-5);
            // Tools expansion must not move the cue.
            let with_tools_open = subtitle_transform(&movie);
            assert!((with_tools_open.position - cue.position).length() < 1e-5);
            assert!(tools.size_m().1 >= transport.size_m().1);
            let delta = Vec3::new(-0.2, 0.1, 0.3);
            let mut moved = movie.clone();
            moved.position += delta;
            assert!((subtitle_transform(&moved).position - cue.position - delta).length() < 1e-5);
        }
    }
    #[test]
    fn transport_click_never_starts_a_screen_grab_or_repeats() {
        for curved in [false, true] {
            let mut controls = ScreenControls::default();
            let mut screen = screen();
            if curved {
                bend(&mut screen, 1.2);
            }
            let original = screen.clone();
            let at = panel_transform(&screen, false)
                .surface_pose(56.0 / 768.0, 152.0 / 200.0)
                .position;
            let mut pointer = input(at, false);
            pointer.is_grabbing = true;
            controls.update(&[pointer], head(), &mut screen, 0.01);
            pointer.is_selecting = true;
            assert_eq!(
                controls.update(&[pointer], head(), &mut screen, 0.01),
                Some(Button::PlayPause)
            );
            assert_eq!(controls.update(&[pointer], head(), &mut screen, 0.01), None);
            assert!(!controls.grab.is_grabbing());
            assert_eq!(screen, original);
        }
    }
    #[test]
    fn bending_and_flattening_preserves_size_and_pose() {
        let mut s = screen();
        let before = s.clone();
        bend(&mut s, 0.3);
        assert_eq!(s.display_mode, DisplayMode::CurvedCylinder);
        assert_eq!(s.size_m(), before.size_m());
        assert_eq!(s.pose(), before.pose());
        let surface = s.surface_pose(0.5, 0.5);
        assert!((surface.position - s.position).length() < 1e-5);
        bend(&mut s, -0.3);
        assert_eq!(s.display_mode, DisplayMode::FlatQuad);
        assert_eq!(s.size_m(), before.size_m());
    }
    #[test]
    fn resize_preserves_aspect_and_suspend_releases_grabs() {
        let mut s = screen();
        resize(&mut s, 0.8);
        assert!((s.size_m().0 / s.size_m().1 - 16.0 / 9.0).abs() < 1e-4);
        let mut c = ScreenControls {
            visible: false,
            ..Default::default()
        };
        c.suspend();
        assert!(c.visible);
        assert!(!c.grab.is_grabbing());
    }
    #[test]
    fn controllers_and_hands_drag_then_release_without_triggering_transport() {
        for source in [
            InteractionSource::Controller(Handedness::Right),
            InteractionSource::Hand(Handedness::Left),
        ] {
            let mut c = ScreenControls {
                visible: false,
                ..Default::default()
            };
            let mut s = screen();
            let target = s.surface_pose(0.5, 1.015).position;
            let mut pointer = input(target, false);
            pointer.source = source;
            c.update(&[pointer], head(), &mut s, 0.01);
            pointer.is_grabbing = true;
            pointer.is_selecting = matches!(source, InteractionSource::Hand(_));
            assert_eq!(c.update(&[pointer], head(), &mut s, 0.01), None);
            assert!(c.grab.is_grabbing(), "{source:?} failed to grab");
            pointer.aim_ray = Some(Ray::new(
                head().position,
                target + Vec3::new(0.6, 0.0, 0.0) - head().position,
            ));
            pointer.grab_pose = Some(ScreenPose::new(
                head().position + Vec3::new(0.3, 0.0, 0.0),
                Quat::IDENTITY,
            ));
            c.update(&[pointer], head(), &mut s, 0.01);
            assert!(s.position.x > 0.1, "{source:?} did not move the screen");
            pointer.is_grabbing = false;
            pointer.is_selecting = false;
            c.update(&[pointer], head(), &mut s, 0.01);
            assert!(!c.grab.is_grabbing());
        }
    }
    #[test]
    fn corner_drag_resizes_both_input_types_and_tracking_loss_releases() {
        for source in [
            InteractionSource::Controller(Handedness::Left),
            InteractionSource::Hand(Handedness::Right),
        ] {
            let mut c = ScreenControls {
                visible: false,
                ..Default::default()
            };
            let mut s = screen();
            let original = s.size_m();
            let target = s.surface_pose(0.96, 0.04).position;
            let mut pointer = input(target, false);
            pointer.source = source;
            c.update(&[pointer], head(), &mut s, 0.01);
            pointer.is_grabbing = true;
            pointer.is_selecting = matches!(source, InteractionSource::Hand(_));
            c.update(&[pointer], head(), &mut s, 0.01);
            assert!(c.grab.resizing().is_some());
            pointer.aim_ray = Some(Ray::new(
                head().position,
                target + Vec3::new(0.3, 0.15, 0.0) - head().position,
            ));
            c.update(&[pointer], head(), &mut s, 0.01);
            assert!(s.size_m().0 > original.0);
            assert!((s.size_m().0 / s.size_m().1 - original.0 / original.1).abs() < 1e-4);
            c.update(&[], head(), &mut s, 0.01);
            assert!(!c.grab.is_grabbing());
        }
    }
    #[test]
    fn timeline_seek_is_bounded_and_curvature_survives_shared_clamping() {
        assert_eq!(
            button_at((24.0 / 768.0, 87.0 / 200.0), false),
            Some(Button::Seek(0))
        );
        assert_eq!(
            button_at((744.0 / 768.0, 87.0 / 200.0), false),
            Some(Button::Seek(1000))
        );
        let mut s = screen();
        bend(&mut s, 0.3);
        let size = s.size_m();
        s.clamp();
        assert!((s.size_m().0 - size.0).abs() < 1e-4);
    }
    #[test]
    fn embedded_surface_moves_with_movie_and_curved_hits_match_artwork() {
        for curved in [false, true] {
            let mut movie = screen();
            if curved {
                bend(&mut movie, 1.2);
            }
            movie.position = Vec3::new(0.7, 1.8, -3.0);
            movie.orientation = Quat::from_euler(0.2, 0.4, 0.1);
            for tools in [false, true] {
                let panel = panel_transform(&movie, tools);
                let viewer =
                    movie.position + movie.orientation.rotate_vec(Vec3::new(0.0, 0.0, 3.0));
                for uv in [(0.1, 0.15), (0.5, 0.5), (0.9, 0.85)] {
                    let target = panel.surface_pose(uv.0, uv.1).position;
                    let hit = panel
                        .intersect_ray(&Ray::new(viewer, target - viewer))
                        .unwrap();
                    assert!((hit.uv.0 - uv.0).abs() < 1e-4 && (hit.uv.1 - uv.1).abs() < 1e-4);
                }
                let local = movie
                    .orientation
                    .conjugate()
                    .rotate_vec(panel.position - movie.position);
                assert!(local.y < 0.0);
                assert!((local.z - 0.004).abs() < 1e-5);
                assert!(panel.size_m().1 <= movie.size_m().1 * 0.84 + 1e-5);
                let delta = Vec3::new(0.3, 0.2, 0.4);
                let mut moved = movie.clone();
                moved.position += delta;
                assert!(
                    (panel_transform(&moved, tools).position - panel.position - delta).length()
                        < 1e-5
                );
                // The corners and move handle stay outside the embedded controls.
                for uv in [
                    (0.02, 0.02),
                    (0.98, 0.02),
                    (0.02, 0.98),
                    (0.98, 0.98),
                    (0.5, 1.015),
                ] {
                    let target = movie.surface_pose(uv.0, uv.1).position;
                    assert!(panel
                        .intersect_ray(&Ray::new(viewer, target - viewer))
                        .is_none());
                }
            }
        }
    }
    #[test]
    fn auto_hide_fades_and_pause_reveals_controls_while_tools_remain_open() {
        let mut c = ScreenControls::default();
        for _ in 0..25 {
            c.advance(true, 0.25);
        }
        assert!(!c.visible);
        assert_eq!(c.opacity, 0.0);
        c.advance(false, 0.1);
        assert!(c.visible);
        assert!(c.opacity > 0.0 && c.opacity < 1.0);
        c.advance(false, 0.1);
        assert_eq!(c.opacity, 1.0);
        c.tools_open = true;
        for _ in 0..40 {
            c.advance(true, 0.25);
        }
        assert!(c.visible);
        c.tools_open = false;
        c.pointing_at_controls = true;
        for _ in 0..40 {
            c.advance(true, 0.25);
        }
        assert!(c.visible);
    }
    #[test]
    fn moving_head_does_not_move_transport_targets_and_screen_menu_is_edge_triggered() {
        let mut c = ScreenControls::default();
        let mut movie = screen();
        let target = panel_transform(&movie, false)
            .surface_pose(628.0 / 768.0, 152.0 / 200.0)
            .position;
        let mut pointer = input(target, false);
        let turned = ScreenPose::new(head().position, Quat::from_euler(0.1, 0.9, 0.0));
        c.update(&[pointer], turned, &mut movie, 0.01);
        pointer.is_selecting = true;
        assert_eq!(
            c.update(&[pointer], turned, &mut movie, 0.01),
            Some(Button::Screen)
        );
        assert!(c.tools_open);
        assert_eq!(c.update(&[pointer], turned, &mut movie, 0.01), None);
        assert!(c.tools_open);
        pointer.is_selecting = false;
        c.update(&[pointer], turned, &mut movie, 0.01);
        pointer.is_selecting = true;
        assert_eq!(
            c.update(&[pointer], turned, &mut movie, 0.01),
            Some(Button::Screen)
        );
        assert!(!c.tools_open);
    }
}
