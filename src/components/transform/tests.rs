//! `world_rotation` / `local_rotation` (#1119): they agree with the rotation
//! glam reads off the child's world matrix, mirrored parent or not, and each
//! undoes the other; `mirrored_direction` turns a local direction onto the
//! line the child's matrix draws (#1125).

use std::f32::consts::FRAC_PI_3;

use glam::{Mat3, Mat4, Quat, Vec3};

use super::{local_rotation, mirrored_direction, world_rotation};

fn same(a: Quat, b: Quat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-5
}

/// Parents with and without a reflection, each rotated off the axes.
fn parents() -> Vec<Mat4> {
    let rot = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-0.4);
    [
        Vec3::ONE,
        Vec3::new(-1.0, 1.0, 1.0),
        Vec3::new(2.0, -2.0, 2.0),
    ]
    .into_iter()
    .map(|s| Mat4::from_scale_rotation_translation(s, rot, Vec3::new(1.0, 2.0, 3.0)))
    .collect()
}

#[test]
fn world_rotation_is_what_the_world_matrix_decomposes_to() {
    let local = Quat::from_rotation_z(FRAC_PI_3) * Quat::from_rotation_x(0.2);
    for parent in parents() {
        let world = parent * Mat4::from_rotation_translation(local, Vec3::X);
        let (scale, decomposed, _) = world.to_scale_rotation_translation();
        let ours = world_rotation(parent, local);
        assert!(same(ours, decomposed), "under {parent:?}");
        assert!(same(local_rotation(parent, ours), local));
        // The reflection is all in the scale: rotation × signed scale is the
        // child's world matrix.
        let rebuilt = Mat3::from_quat(ours) * Mat3::from_diagonal(scale);
        assert!(rebuilt.abs_diff_eq(Mat3::from_mat4(world), 1e-5));
    }
}

#[test]
fn an_unmirrored_parent_is_the_plain_product() {
    let parent = parents()[0];
    let (_, parent_rot, _) = parent.to_scale_rotation_translation();
    let local = Quat::from_rotation_z(FRAC_PI_3);
    assert_eq!(
        world_rotation(parent, local),
        (parent_rot * local).normalize()
    );
    assert_eq!(
        local_rotation(parent, local),
        (parent_rot.inverse() * local).normalize()
    );
}

#[test]
fn a_mirrored_direction_lands_on_the_drawn_world_direction() {
    let local = Quat::from_rotation_z(FRAC_PI_3) * Quat::from_rotation_x(0.2);
    let dir = Vec3::new(1.0, 1.0, 0.0).normalize();
    for parent in parents() {
        let world = parent * Mat4::from_quat(local);
        let drawn = world.transform_vector3(dir).normalize();
        let ours = world_rotation(parent, local) * mirrored_direction(parent, dir);
        assert!(ours.abs_diff_eq(drawn, 1e-5), "under {parent:?}");
    }
    assert_eq!(mirrored_direction(parents()[0], dir), dir);
}
