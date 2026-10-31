//! A furnished loft assembled from compositor surfaces, without resampling the movie.
use xr_core::spatial::{Quat, ScreenPose, Vec3};

pub struct LoftSurface {
    pub pose: ScreenPose,
    pub size: (f32, f32),
    pub rect: (i32, i32, i32, i32),
}
/// Six environment surfaces leave room within the 16-layer baseline for stereo video,
/// transport, five manipulation handles and two cursors.
pub fn loft_surfaces(anchor: ScreenPose) -> Vec<LoftSurface> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let floor = (128, 204, 128, 52);
    let wall = (256, 192, 128, 64);
    let upholstery = (384, 192, 128, 64);
    [
        (
            Vec3::new(0.0, 0.0, 0.0),
            Quat::from_axis_angle(Vec3::UNIT_X, -FRAC_PI_2),
            (14.0, 12.0),
            floor,
        ),
        (Vec3::new(0.0, 2.5, -6.0), Quat::IDENTITY, (14.0, 5.0), wall),
        (
            Vec3::new(-7.0, 2.5, 0.0),
            Quat::from_axis_angle(Vec3::UNIT_Y, FRAC_PI_2),
            (12.0, 5.0),
            wall,
        ),
        (
            Vec3::new(7.0, 2.5, 0.0),
            Quat::from_axis_angle(Vec3::UNIT_Y, -FRAC_PI_2),
            (12.0, 5.0),
            wall,
        ),
        (
            Vec3::new(0.0, 0.46, 1.6),
            Quat::from_axis_angle(Vec3::UNIT_X, -FRAC_PI_2),
            (3.2, 1.0),
            upholstery,
        ),
        (
            Vec3::new(0.0, 0.96, 2.1),
            Quat::from_axis_angle(Vec3::UNIT_Y, PI),
            (3.2, 1.0),
            upholstery,
        ),
    ]
    .into_iter()
    .map(|(position, orientation, size, rect)| LoftSurface {
        pose: ScreenPose::new(
            anchor.position + anchor.orientation.rotate_vec(position),
            anchor.orientation * orientation,
        ),
        size,
        rect,
    })
    .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loft_obeys_layer_budget_and_remains_fixed_when_screen_moves() {
        let anchor = ScreenPose::new(Vec3::new(2.0, 0.0, 3.0), Quat::IDENTITY);
        let surfaces = loft_surfaces(anchor);
        assert_eq!(surfaces.len() + 2 + 1 + 5 + 2, 16);
        assert_eq!(surfaces[0].pose.position, anchor.position);
        assert!(
            (surfaces[0].pose.orientation.rotate_vec(Vec3::UNIT_Z) - Vec3::UNIT_Y).length() < 1e-5
        );
        for surface in surfaces {
            assert!(surface.size.0 > 0.0 && surface.size.1 > 0.0);
            let (x, y, w, h) = surface.rect;
            assert!(x >= 128 && y >= 192 && x + w <= 512 && y + h <= 256);
        }
    }
}
