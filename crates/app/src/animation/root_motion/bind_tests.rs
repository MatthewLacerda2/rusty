//! Yaw measured from the bind facing (#1745): hips bound flipped 180° about a
//! horizontal axis (a Z-up export) still turn the mover, and the applied pose
//! holds them at that bind facing.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Mat4, Quat};

use super::super::sample_pose;
use super::super::test_rig::*;
use super::tests::{run, total};
use super::RootBone;
use rusty_asset::anim_data::{Interpolation, Track};
use rusty_components::{MeshComponent, Motion};

/// The rig with its hips bound flipped about X, turning a quarter about the
/// entity's +Y over one second.
fn flipped() -> MeshComponent {
    let bind = Quat::from_rotation_x(PI);
    let mut clip = slide("Turn", &[], 0.0, 0.0, 1.0);
    clip.tracks[HIPS].rotation = Track {
        times: vec![0.0, 1.0],
        values: vec![bind, Quat::from_rotation_y(FRAC_PI_2) * bind],
        interpolation: Interpolation::Linear,
    };
    clip.recompute_duration();
    let mut mesh = mesh(vec![clip]);
    let skin = mesh.skin.as_mut().expect("skinned");
    skin.local_bind[HIPS].rotation = bind;
    skin.bind_global[HIPS] = Mat4::from_quat(bind);
    mesh
}

#[test]
fn a_flipped_bind_still_measures_the_turn() {
    let mesh = flipped();
    let mut anim = animator(Motion::Clip("Turn"), None, 0.0);
    let done = total(&run(&mut anim, &mesh, 60));
    assert!((done.yaw - FRAC_PI_2).abs() < 1e-3, "{done:?}");
}

#[test]
fn the_stripped_pose_holds_a_flipped_bind_facing() {
    let mesh = flipped();
    let mut anim = animator(Motion::Clip("Turn"), None, 0.5);
    anim.apply_root_motion = true;
    let (skin, mut pose) = sample_pose(&anim, &mesh, None).expect("a skinned rig");
    RootBone::resolve(skin, "")
        .expect("hips")
        .strip(skin, &mut pose);
    let held = pose[HIPS].expect("keyed").rotation;
    let bind = Quat::from_rotation_x(PI);
    assert!(held.dot(bind).abs() > 1.0 - 1e-4, "{held:?}");
}
