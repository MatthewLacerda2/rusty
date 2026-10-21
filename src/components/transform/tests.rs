//! `world_rotation` / `local_rotation` (#1119): they agree with the rotation
//! glam reads off the child's world matrix, mirrored parent or not, and each
//! undoes the other; a local direction and a local frame seen through the
//! mirror the child's signed scale carries land where its matrix draws them
//! (#1125).

use std::f32::consts::FRAC_PI_3;

use glam::{Mat3, Mat4, Quat, Vec3};

use super::{local_rotation, mirror, mirrored_direction, mirrored_frame_axes, world_rotation};

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

/// Own scales with and without a reflection, on X and off it.
const OWN: [Vec3; 3] = [
    Vec3::ONE,
    Vec3::new(1.0, 1.0, -1.0),
    Vec3::new(-1.0, 1.0, 1.0),
];

/// A child's world matrix's linear part, and its world rotation and mirror.
fn child(parent: Mat4, local: Quat, own: Vec3) -> (Mat3, Quat, Vec3) {
    let world = parent * Mat4::from_scale_rotation_translation(own, local, Vec3::X);
    let mirror = mirror(parent.to_scale_rotation_translation().0 * own);
    (
        Mat3::from_mat4(world),
        world_rotation(parent, local),
        mirror,
    )
}

/// The rotation whose X is `x` and Y is `y` made perpendicular to it.
fn basis(x: Vec3, y: Vec3) -> Mat3 {
    let x = x.normalize();
    let y = (y - x * y.dot(x)).normalize();
    Mat3::from_cols(x, y, x.cross(y))
}

#[test]
fn a_mirrored_direction_lands_on_the_drawn_world_direction() {
    let local = Quat::from_rotation_z(FRAC_PI_3) * Quat::from_rotation_x(0.2);
    let dir = Vec3::new(1.0, 1.0, 0.0).normalize();
    for parent in parents() {
        for own in OWN {
            let (world, rot, mirror) = child(parent, local, own);
            let drawn = (world * dir).normalize();
            let ours = rot * mirrored_direction(mirror, dir);
            assert!(ours.abs_diff_eq(drawn, 1e-5), "{own} under {parent:?}");
        }
    }
    assert_eq!(mirrored_direction(Vec3::ONE, dir), dir);
}

/// A frame seen through the mirror turns the way the mirror image does: a twist
/// about its X, drawn through the child's matrix, is the twist about the world
/// frame's X, and that X lies on the drawn line in the mirror-image sense.
#[test]
fn a_mirrored_frame_twists_as_the_drawn_frame_does() {
    let local = Quat::from_rotation_y(0.9) * Quat::from_rotation_x(-0.3);
    let (x, y) = (Vec3::new(1.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 1.0));
    let twist = Mat3::from_rotation_x(0.6);
    for parent in parents() {
        for own in OWN {
            let (world, rot, mirror) = child(parent, local, own);
            let (mx, my) = mirrored_frame_axes(mirror, x, y);
            let frame = Mat3::from_quat(rot) * basis(mx, my);
            let b = basis(x, y);
            let drawn = world * b * twist * b.inverse() * world.inverse();
            let ours = frame * twist * frame.inverse();
            assert!(ours.abs_diff_eq(drawn, 1e-5), "{own} under {parent:?}");
            let reflects = world.determinant() < 0.0;
            let sense = if reflects { -1.0 } else { 1.0 };
            let line = (world * x).normalize() * sense;
            assert!(
                frame.x_axis.abs_diff_eq(line, 1e-5),
                "{own} under {parent:?}"
            );
            assert_eq!(
                mirrored_frame_axes(mirror, mx, my),
                (x, y),
                "its own inverse"
            );
        }
    }
    assert_eq!(mirrored_frame_axes(Vec3::ONE, x, y), (x, y));
}
