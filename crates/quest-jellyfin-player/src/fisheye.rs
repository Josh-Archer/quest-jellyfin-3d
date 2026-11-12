//! Equidistant fisheye lens remap onto the unit sphere.
//!
//! OpenXR has no fisheye composition layer. Classified Fisheye items therefore
//! take a dedicated compositor path that builds an equidistant UV mesh and
//! submits hemispherical `CompositionLayerEquirect2KHR` layers as the sphere
//! carrier (see `projection::build_fisheye_remap_layers`).
//!
//! Lens model: equidistant (`r = f · θ`), with the optical axis along OpenXR
//! forward (−Z). The image circle is inscribed in each eye's normalized crop
//! (`radius = 0.5` in local UV). Default FOV is 180° (π rad).

use std::f32::consts::{FRAC_PI_2, PI};

/// Equidistant circular fisheye covering `fov_radians` across the image circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquidistantFisheye {
    /// Full field of view across the diameter of the image circle, in radians.
    pub fov_radians: f32,
}

impl Default for EquidistantFisheye {
    fn default() -> Self {
        Self {
            fov_radians: PI, // 180° dual-fisheye / VR180 camera default
        }
    }
}

impl EquidistantFisheye {
    pub fn new(fov_radians: f32) -> Option<Self> {
        if !fov_radians.is_finite() || fov_radians <= 0.0 || fov_radians > PI + 1e-3 {
            return None;
        }
        Some(Self { fov_radians })
    }

    pub fn half_fov(self) -> f32 {
        self.fov_radians * 0.5
    }

    /// Maps a unit-length view direction to local eye-crop UV in `[0, 1]²`.
    ///
    /// Returns `None` when the direction is outside the lens FOV (beyond the
    /// image circle). The crop center `(0.5, 0.5)` is the optical axis (−Z).
    pub fn direction_to_uv(self, direction: [f32; 3]) -> Option<(f32, f32)> {
        let [x, y, z] = normalize(direction)?;
        // Angle from optical axis (−Z): cos(θ) = −z.
        let cos_theta = (-z).clamp(-1.0, 1.0);
        let theta = cos_theta.acos();
        if theta > self.half_fov() + 1e-4 {
            return None;
        }
        let r_norm = if self.half_fov() > 0.0 {
            theta / self.half_fov()
        } else {
            0.0
        };
        // Azimuth in the plane perpendicular to −Z.
        let phi = y.atan2(x);
        let (s, c) = phi.sin_cos();
        let u = 0.5 + 0.5 * r_norm * c;
        let v = 0.5 + 0.5 * r_norm * s;
        Some((u, v))
    }

    /// Inverse of [`direction_to_uv`]: local eye-crop UV → unit direction.
    ///
    /// Returns `None` when the UV falls outside the inscribed image circle.
    pub fn uv_to_direction(self, u: f32, v: f32) -> Option<[f32; 3]> {
        if !u.is_finite() || !v.is_finite() {
            return None;
        }
        let dx = (u - 0.5) * 2.0;
        let dy = (v - 0.5) * 2.0;
        let r_norm = (dx * dx + dy * dy).sqrt();
        if r_norm > 1.0 + 1e-4 {
            return None;
        }
        let theta = r_norm * self.half_fov();
        let phi = dy.atan2(dx);
        let (sin_theta, cos_theta) = theta.sin_cos();
        let (sin_phi, cos_phi) = phi.sin_cos();
        // Optical axis −Z; θ tilts toward XY.
        Some([sin_theta * cos_phi, sin_theta * sin_phi, -cos_theta])
    }
}

/// One vertex of the hemispherical remap mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FisheyeMeshVertex {
    /// Unit-sphere position (viewer at the origin).
    pub position: [f32; 3],
    /// Local eye-crop UV from the equidistant lens map.
    pub uv: [f32; 2],
}

/// Triangle mesh that samples a fisheye eye crop onto the forward hemisphere.
#[derive(Debug, Clone, PartialEq)]
pub struct FisheyeRemapMesh {
    pub lens: EquidistantFisheye,
    pub vertices: Vec<FisheyeMeshVertex>,
    /// Triangle indices into `vertices` (CCW when viewed from the origin).
    pub indices: Vec<u32>,
}

impl FisheyeRemapMesh {
    /// Builds a UV remap mesh for `lens` over the forward hemisphere.
    ///
    /// `rings` is the number of latitude subdivisions (≥ 1); `segments` is the
    /// number of azimuth samples per ring (≥ 3).
    pub fn hemisphere(lens: EquidistantFisheye, rings: u32, segments: u32) -> Self {
        let rings = rings.max(1);
        let segments = segments.max(3);
        let max_theta = lens.half_fov().min(FRAC_PI_2);
        let mut vertices = Vec::with_capacity((rings * segments + 1) as usize);
        // Pole (optical axis).
        let pole_uv = lens.direction_to_uv([0.0, 0.0, -1.0]).unwrap_or((0.5, 0.5));
        vertices.push(FisheyeMeshVertex {
            position: [0.0, 0.0, -1.0],
            uv: [pole_uv.0, pole_uv.1],
        });
        for ring in 1..=rings {
            let t = ring as f32 / rings as f32;
            let theta = t * max_theta;
            let (sin_theta, cos_theta) = theta.sin_cos();
            for seg in 0..segments {
                let phi = (seg as f32 / segments as f32) * std::f32::consts::TAU;
                let (sin_phi, cos_phi) = phi.sin_cos();
                let position = [sin_theta * cos_phi, sin_theta * sin_phi, -cos_theta];
                let uv = lens.direction_to_uv(position).unwrap_or((0.5, 0.5));
                vertices.push(FisheyeMeshVertex {
                    position,
                    uv: [uv.0, uv.1],
                });
            }
        }
        let mut indices = Vec::new();
        // Fan from the pole to the first ring.
        for seg in 0..segments {
            let a = 1 + seg;
            let b = 1 + (seg + 1) % segments;
            indices.extend_from_slice(&[0, b, a]);
        }
        // Quads between consecutive rings, as two triangles each.
        for ring in 1..rings {
            let row0 = 1 + (ring - 1) * segments;
            let row1 = 1 + ring * segments;
            for seg in 0..segments {
                let s0 = seg;
                let s1 = (seg + 1) % segments;
                let i00 = row0 + s0;
                let i01 = row0 + s1;
                let i10 = row1 + s0;
                let i11 = row1 + s1;
                indices.extend_from_slice(&[i00, i01, i11, i00, i11, i10]);
            }
        }
        Self {
            lens,
            vertices,
            indices,
        }
    }
}

/// How spherical cinema presents a decoded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SphericalSubmission {
    /// Longitude/latitude sampling via `CompositionLayerEquirect2KHR`.
    Equirect180,
    /// Full-sphere longitude/latitude sampling.
    Equirect360,
    /// Equidistant polar UV mesh remap (not flat, not plain VR180 equirect).
    FisheyeEquidistantRemap,
}

/// Resolves the compositor submission kind for a projection choice.
///
/// Fisheye never collapses to a flat quad or to plain VR180 equirect.
pub fn spherical_submission(projection: crate::Projection) -> Option<SphericalSubmission> {
    match projection {
        crate::Projection::Equirectangular180 => Some(SphericalSubmission::Equirect180),
        crate::Projection::Equirectangular360 => Some(SphericalSubmission::Equirect360),
        crate::Projection::Fisheye => Some(SphericalSubmission::FisheyeEquidistantRemap),
        crate::Projection::Flat => None,
    }
}

fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if !len_sq.is_finite() || len_sq <= 0.0 {
        return None;
    }
    let inv = 1.0 / len_sq.sqrt();
    Some([v[0] * inv, v[1] * inv, v[2] * inv])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_pixel_maps_to_optical_axis() {
        let lens = EquidistantFisheye::default();
        let uv = lens.direction_to_uv([0.0, 0.0, -1.0]).unwrap();
        assert!((uv.0 - 0.5).abs() < 1e-5);
        assert!((uv.1 - 0.5).abs() < 1e-5);
        let back = lens.uv_to_direction(0.5, 0.5).unwrap();
        assert!((back[0]).abs() < 1e-5);
        assert!((back[1]).abs() < 1e-5);
        assert!((back[2] + 1.0).abs() < 1e-5);
    }

    #[test]
    fn image_circle_edge_is_half_fov() {
        let lens = EquidistantFisheye::default(); // 180° → edge at 90°
        let right = lens.direction_to_uv([1.0, 0.0, 0.0]).unwrap();
        assert!((right.0 - 1.0).abs() < 1e-4, "u={}", right.0);
        assert!((right.1 - 0.5).abs() < 1e-4, "v={}", right.1);
        let up = lens.direction_to_uv([0.0, 1.0, 0.0]).unwrap();
        assert!((up.0 - 0.5).abs() < 1e-4);
        assert!((up.1 - 1.0).abs() < 1e-4);
        // Outside the forward hemisphere / FOV.
        assert!(lens.direction_to_uv([0.0, 0.0, 1.0]).is_none());
        assert!(lens.uv_to_direction(0.0, 0.0).is_none()); // corner outside circle
    }

    #[test]
    fn known_forty_five_degree_angle_is_halfway_to_edge() {
        let lens = EquidistantFisheye::default(); // half-FOV = 90°
        let theta = FRAC_PI_2 * 0.5; // 45°
        let (s, c) = theta.sin_cos();
        let dir = [s, 0.0, -c]; // in the XZ plane
        let uv = lens.direction_to_uv(dir).unwrap();
        // r_norm = 45/90 = 0.5 → u = 0.5 + 0.25
        assert!((uv.0 - 0.75).abs() < 1e-4, "u={}", uv.0);
        assert!((uv.1 - 0.5).abs() < 1e-4, "v={}", uv.1);
        let back = lens.uv_to_direction(uv.0, uv.1).unwrap();
        let dot = back[0] * dir[0] + back[1] * dir[1] + back[2] * dir[2];
        assert!((dot - 1.0).abs() < 1e-4, "round-trip direction");
    }

    #[test]
    fn hemisphere_mesh_covers_pole_and_equator_uvs() {
        let lens = EquidistantFisheye::default();
        let mesh = FisheyeRemapMesh::hemisphere(lens, 4, 8);
        assert!(!mesh.vertices.is_empty());
        assert_eq!(mesh.vertices[0].position, [0.0, 0.0, -1.0]);
        assert!((mesh.vertices[0].uv[0] - 0.5).abs() < 1e-5);
        assert!(mesh.indices.len() >= 3);
        assert_eq!(mesh.indices.len() % 3, 0);
        // Outer ring should reach the image-circle edge (r_norm ≈ 1).
        let outer = &mesh.vertices[mesh.vertices.len() - 8..];
        let radii: Vec<f32> = outer
            .iter()
            .map(|v| {
                let dx = (v.uv[0] - 0.5) * 2.0;
                let dy = (v.uv[1] - 0.5) * 2.0;
                (dx * dx + dy * dy).sqrt()
            })
            .collect();
        assert!(
            radii.iter().all(|r| (*r - 1.0).abs() < 1e-3),
            "outer UVs should sit on the image circle: {radii:?}"
        );
    }

    #[test]
    fn spherical_submission_distinguishes_fisheye_from_vr180_and_flat() {
        use crate::Projection;
        assert_eq!(
            spherical_submission(Projection::Fisheye),
            Some(SphericalSubmission::FisheyeEquidistantRemap)
        );
        assert_ne!(
            spherical_submission(Projection::Fisheye),
            spherical_submission(Projection::Equirectangular180)
        );
        assert_eq!(
            spherical_submission(Projection::Equirectangular180),
            Some(SphericalSubmission::Equirect180)
        );
        assert_eq!(
            spherical_submission(Projection::Equirectangular360),
            Some(SphericalSubmission::Equirect360)
        );
        assert_eq!(spherical_submission(Projection::Flat), None);
    }
}
