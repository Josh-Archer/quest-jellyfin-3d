//! Shared DeskQuest manipulation, with transport hit targets kept separate from screen grabs.
use xr_core::{
    spatial::{
        ray_intersects_quad, GrabStateMachine, InteractionEvent, InteractionInput,
        InteractionSource, ScreenPose, ScreenTransform, Vec3,
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
    Exit,
    Smaller,
    Larger,
    Flatter,
    Curvier,
    Recenter,
    Hide,
}

/// Matches the 512 x 192 transport artwork in VrActivity (the remaining pixels are an atlas).
pub fn button_at((u, v): (f32, f32)) -> Option<Button> {
    if !u.is_finite() || !v.is_finite() || !(0.0..1.0).contains(&u) {
        return None;
    }
    let x = u * 512.0;
    let y = v * 192.0;
    if (32.0..54.0).contains(&y) && (12.0..=500.0).contains(&x) {
        return Some(Button::Seek(((x - 12.0) / 488.0 * 1000.0).round() as u16));
    }
    if (4.0..40.0).contains(&y) && x >= 448.0 {
        return Some(Button::Hide);
    }
    let row = if (60.0..100.0).contains(&y) {
        [
            Button::PlayPause,
            Button::Back,
            Button::Forward,
            Button::Audio,
            Button::Exit,
        ]
    } else if (106.0..146.0).contains(&y) {
        [
            Button::Smaller,
            Button::Larger,
            Button::Flatter,
            Button::Curvier,
            Button::Recenter,
        ]
    } else {
        return None;
    };
    let cell = (x / 102.4) as usize;
    let inside = x - cell as f32 * 102.4;
    (4.0..98.4).contains(&inside).then(|| row[cell])
}

pub fn panel_pose(head: ScreenPose) -> ScreenPose {
    ScreenPose::new(
        head.position + head.orientation.rotate_vec(Vec3::new(0.0, -0.46, -1.45)),
        head.orientation,
    )
}
pub const PANEL_SIZE: (f32, f32) = (1.35, 1.35 * 192.0 / 512.0);

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
    pressed: [bool; 4],
}
impl Default for ScreenControls {
    fn default() -> Self {
        Self {
            grab: GrabStateMachine::new(),
            visible: true,
            pressed: [false; 4],
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
        self.visible = true;
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
        let mut candidates = Vec::new();
        let mut active = self.grab.grab_session().map(|s| s.source);
        if active.is_some_and(|owner| !inputs.iter().any(|i| i.source == owner)) {
            self.grab.release();
            active = None;
        }
        for input in inputs {
            let index = source_index(input.source);
            let click = input.is_selecting && !self.pressed[index];
            self.pressed[index] = input.is_selecting;
            let panel_hit = input.aim_ray.as_ref().and_then(|ray| {
                self.visible
                    .then(|| ray_intersects_quad(ray, &panel_pose(head), PANEL_SIZE))
                    .flatten()
            });
            let mut candidate = *input;
            if panel_hit.is_some() && active.is_none() {
                candidate.is_grabbing = false;
                candidate.aim_ray = None;
                if click && action.is_none() {
                    action = panel_hit.and_then(|hit| button_at(hit.uv));
                }
            } else {
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
        if action == Some(Button::Hide) {
            self.visible = false;
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
    fn every_artwork_button_has_one_target_and_gaps_are_inert() {
        for (row, expected) in [
            (
                80.0,
                [
                    Button::PlayPause,
                    Button::Back,
                    Button::Forward,
                    Button::Audio,
                    Button::Exit,
                ],
            ),
            (
                126.0,
                [
                    Button::Smaller,
                    Button::Larger,
                    Button::Flatter,
                    Button::Curvier,
                    Button::Recenter,
                ],
            ),
        ] {
            for (column, button) in expected.into_iter().enumerate() {
                assert_eq!(
                    button_at(((column as f32 + 0.5) / 5.0, row / 192.0)),
                    Some(button)
                );
            }
        }
        for uv in [
            (0.0, 0.4),
            (0.5, 0.29),
            (f32::NAN, 0.4),
            (1.0, 0.4),
            (0.5, f32::INFINITY),
        ] {
            assert_eq!(button_at(uv), None);
        }
    }
    #[test]
    fn transport_click_never_starts_a_screen_grab_or_repeats() {
        let mut controls = ScreenControls::default();
        let mut screen = screen();
        let original = screen.clone();
        let at = panel_pose(head()).position
            + Vec3::new(
                -PANEL_SIZE.0 * 0.4,
                PANEL_SIZE.1 * (0.5 - 80.0 / 192.0),
                0.0,
            );
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
            button_at((12.0 / 512.0, 43.0 / 192.0)),
            Some(Button::Seek(0))
        );
        assert_eq!(
            button_at((500.0 / 512.0, 43.0 / 192.0)),
            Some(Button::Seek(1000))
        );
        let mut s = screen();
        bend(&mut s, 0.3);
        let size = s.size_m();
        s.clamp();
        assert!((s.size_m().0 - size.0).abs() < 1e-4);
    }
}
