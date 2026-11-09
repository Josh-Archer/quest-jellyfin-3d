//! Cinema subtitle controls and cue placement, through the same input path as playback.
use quest_jellyfin_player::screen_controls::{
    bend, panel_transform, subtitle_transform, Button, ScreenControls,
};
use xr_core::spatial::{
    Handedness, InteractionInput, InteractionSource, Quat, Ray, ScreenPose, ScreenTransform, Vec3,
};
use xr_core::DisplayMode;

fn movie() -> ScreenTransform {
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

fn pointer(at: Vec3, select: bool) -> InteractionInput {
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
fn subs_press_keeps_the_movie_still_and_the_cue_follows_a_later_grab() {
    for curved in [false, true] {
        let mut controls = ScreenControls::default();
        let mut screen = movie();
        if curved {
            bend(&mut screen, 1.2);
        }
        let parked = screen.clone();
        let at = panel_transform(&screen, false)
            .surface_pose(520.0 / 768.0, 152.0 / 200.0)
            .position;
        let mut aim = pointer(at, false);
        assert_eq!(controls.update(&[aim], head(), &mut screen, 0.01), None);
        aim.is_selecting = true;
        assert_eq!(
            controls.update(&[aim], head(), &mut screen, 0.01),
            Some(Button::Subtitles)
        );
        assert_eq!(screen, parked, "a Subs press must not grab or resize");
        assert!(!controls.grab.is_grabbing());

        // Chrome can be hidden. The cue is content, so it still tracks the movie.
        controls.visible = false;
        let cue_before = subtitle_transform(&screen);
        let transport = panel_transform(&screen, false);
        let local = screen
            .orientation
            .conjugate()
            .rotate_vec(cue_before.position - screen.position);
        let transport_local = screen
            .orientation
            .conjugate()
            .rotate_vec(transport.position - screen.position);
        assert!(local.y > transport_local.y);
        assert!((local.z - 0.004).abs() < 1e-4);

        let edge = screen.surface_pose(0.5, 1.015).position;
        let mut grab = pointer(edge, false);
        controls.update(&[grab], head(), &mut screen, 0.01);
        grab.is_grabbing = true;
        assert_eq!(controls.update(&[grab], head(), &mut screen, 0.01), None);
        assert!(controls.grab.is_grabbing());
        let shift = Vec3::new(0.35, -0.1, 0.2);
        grab.aim_ray = Some(Ray::new(head().position, edge + shift - head().position));
        grab.grab_pose = Some(ScreenPose::new(head().position + shift, Quat::IDENTITY));
        controls.update(&[grab], head(), &mut screen, 0.01);
        let moved = screen.position - parked.position;
        assert!(moved.length() > 0.05, "the grab should move the movie");
        let cue_shift = subtitle_transform(&screen).position - cue_before.position;
        assert!(
            (cue_shift - moved).length() < 1e-3,
            "the cue must move with the movie, not stay in the room"
        );
    }
}
