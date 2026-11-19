//! Where `hiz_clusters.wgsl`'s cone test looks from (#1968): the centre of
//! projection of the camera's matrix.

use glam::{Mat4, Vec4};

/// The eye `hiz_clusters.wgsl` faces clusters against: `view_proj`'s centre of
/// projection, which `(0, 0, 1, 0)` in clip space maps back to — a point (`w` 1)
/// for a perspective camera, the view direction (`w` 0) for an orthographic one.
pub(super) fn eye_of(view_proj: Mat4) -> Vec4 {
    let e = view_proj.inverse() * Vec4::new(0.0, 0.0, 1.0, 0.0);
    if e.w.abs() > 1e-6 * e.truncate().length().max(1e-30) {
        (e.truncate() / e.w).extend(1.0)
    } else {
        e.truncate().normalize_or_zero().extend(0.0)
    }
}

#[cfg(test)]
mod tests {
    use glam::camera::rh::{proj::directx, view::look_at_mat4};
    use glam::Vec3;

    use super::eye_of;

    #[test]
    fn the_eye_is_the_camera_for_a_perspective_and_its_direction_for_an_ortho() {
        let view = look_at_mat4(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, -5.0), Vec3::Y);
        let persp = directx::perspective(1.0, 1.5, 0.1, 100.0) * view;
        let e = eye_of(persp);
        assert!(
            (e.truncate() - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-3 && e.w == 1.0,
            "{e}"
        );
        let ortho = directx::orthographic(-2.0, 2.0, -2.0, 2.0, 0.1, 100.0) * view;
        let e = eye_of(ortho);
        assert!(
            (e.truncate() - Vec3::NEG_Z).length() < 1e-4 && e.w == 0.0,
            "{e}"
        );
    }
}
