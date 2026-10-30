//! Place a cinema in front of the tracked viewer, then leave it anchored in the room.
use xr_core::spatial::{ScreenPose, Vec3};

pub fn pose_in_front(head: ScreenPose, distance: f32) -> ScreenPose {
    let orientation = head.orientation.normalize();
    let forward = orientation.rotate_vec(Vec3::new(0.0, 0.0, -1.0));
    ScreenPose::new(head.position + forward * distance, orientation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xr_core::spatial::Quat;
    #[test]
    fn cinema_is_in_front_of_a_translated_and_turned_viewer() {
        let head = ScreenPose::new(
            Vec3::new(8.0, 1.1, 3.0),
            Quat::from_euler(0.0, std::f32::consts::FRAC_PI_2, 0.0),
        );
        let cinema = pose_in_front(head, 2.0);
        assert!((cinema.position.x - 6.0).abs() < 0.0001);
        assert!((cinema.position.y - 1.1).abs() < 0.0001);
        assert!((cinema.position.z - 3.0).abs() < 0.0001);
        assert!((cinema.orientation.y - head.orientation.y).abs() < 0.0001);
        assert!((cinema.orientation.w - head.orientation.w).abs() < 0.0001);
    }
    #[test]
    fn cinema_respects_reclined_viewing_direction() {
        let head = ScreenPose::new(
            Vec3::new(0.0, 1.0, 0.0),
            Quat::from_euler(std::f32::consts::FRAC_PI_4, 0.0, 0.0),
        );
        let cinema = pose_in_front(head, 2.0);
        assert!((cinema.position.y - (1.0 + 2.0_f32.sqrt())).abs() < 0.0001);
        assert!((cinema.position.z + 2.0_f32.sqrt()).abs() < 0.0001);
    }
}
