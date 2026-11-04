//! Root motion over one clip (#1745): the travel adds up to the clip's, a loop's
//! wrap and a reversed or scaled playhead don't jump, a pause holds, a turn
//! curves the path, and the applied pose holds the bone in place.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};

use super::super::test_rig::*;
use super::super::{sample_pose, step};
use super::RootBone;
use crate::asset::anim_data::{Interpolation, Track};
use crate::components::{AnimatorComponent, MeshComponent, Motion, RootMotion};

pub(super) const DT: f32 = 1.0 / 60.0;

/// Step `anim` `n` times; every step's root motion, in order.
pub(super) fn run(anim: &mut AnimatorComponent, mesh: &MeshComponent, n: usize) -> Vec<RootMotion> {
    (0..n)
        .map(|_| step(anim, Some(mesh), None, DT).root.unwrap_or_default())
        .collect()
}

/// The steps chained: where the mover ends up and which way it faces.
pub(super) fn total(steps: &[RootMotion]) -> RootMotion {
    steps.iter().fold(RootMotion::ZERO, |acc, &m| acc.then(m))
}

/// The lunge: hips travel 2 m along +X over one second.
fn lunge() -> MeshComponent {
    mesh(vec![slide("Lunge", &[HIPS], 0.0, 2.0, 1.0)])
}

fn playing(clip: &str, looping: bool) -> AnimatorComponent {
    let mut anim = animator(Motion::Clip(clip), None, 0.0);
    anim.base.loop_clip = looping;
    anim
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-3
}

#[test]
fn a_clip_whose_hips_move_two_metres_moves_two_metres_and_stops_at_its_end() {
    let mut anim = playing("Lunge", false);
    let steps = run(&mut anim, &lunge(), 90);
    assert!(
        close(total(&steps).position, Vec3::X * 2.0),
        "{:?}",
        total(&steps)
    );
    assert!(
        steps[70..].iter().all(RootMotion::is_zero),
        "clamped at the end"
    );
}

#[test]
fn a_loop_keeps_travelling_across_its_wrap() {
    let mut anim = playing("Lunge", true);
    let steps = run(&mut anim, &lunge(), 150);
    assert!(close(total(&steps).position, Vec3::X * 5.0));
    let per_step = 2.0 * DT;
    for m in &steps {
        assert!(
            (m.position.x - per_step).abs() < 1e-3,
            "no jump at the wrap: {m:?}"
        );
    }
}

#[test]
fn speed_scales_and_reverses_the_travel() {
    let mesh = lunge();
    let mut fast = playing("Lunge", true);
    fast.speed = 2.5;
    assert!(close(
        total(&run(&mut fast, &mesh, 60)).position,
        Vec3::X * 5.0
    ));
    let mut back = playing("Lunge", true);
    back.speed = -1.0;
    assert!(close(
        total(&run(&mut back, &mesh, 90)).position,
        Vec3::X * -3.0
    ));
}

#[test]
fn a_paused_animator_holds_and_resumes_without_a_jump() {
    let mesh = lunge();
    let mut anim = playing("Lunge", false);
    let before = total(&run(&mut anim, &mesh, 30));
    anim.freeze = true;
    assert!(run(&mut anim, &mesh, 20).iter().all(RootMotion::is_zero));
    anim.freeze = false;
    let after = total(&run(&mut anim, &mesh, 40));
    assert!(close(before.then(after).position, Vec3::X * 2.0));
}

#[test]
fn a_turning_clip_turns_the_mover_and_retraces_the_clip_path() {
    // The hips move 1 m along +X while turning a quarter about +Y.
    let mut clip = slide("Arc", &[HIPS], 0.0, 1.0, 1.0);
    clip.tracks[HIPS].rotation = Track {
        times: vec![0.0, 1.0],
        values: vec![Quat::IDENTITY, Quat::from_rotation_y(FRAC_PI_2)],
        interpolation: Interpolation::Linear,
    };
    let mesh = mesh(vec![clip]);
    let mut anim = playing("Arc", false);
    let steps = run(&mut anim, &mesh, 60);
    let done = total(&steps);
    assert!((done.yaw - FRAC_PI_2).abs() < 1e-3, "{done:?}");
    // Each step is measured in the frame the hips faced, so chained onto the
    // turning mover the path is the clip's own: 1 m along +X.
    assert!(close(done.position, Vec3::X), "{done:?}");
    assert!(
        steps[50].position.z > 1e-4,
        "late steps read sideways in the turned frame"
    );
}

#[test]
fn applied_root_motion_holds_the_bone_horizontally_and_keeps_its_height() {
    let mut clip = slide("Hop", &[HIPS], 0.0, 2.0, 1.0);
    clip.tracks[HIPS].translation.values = vec![Vec3::ZERO, Vec3::new(2.0, 0.5, 1.0)];
    let mesh = mesh(vec![clip]);
    let mut anim = playing("Hop", false);
    anim.base.time = 0.5;
    let at = |anim: &AnimatorComponent| {
        let (skin, mut pose) = sample_pose(anim, &mesh, None).expect("a skinned rig");
        if anim.apply_root_motion {
            RootBone::resolve(skin, "")
                .expect("hips")
                .strip(skin, &mut pose);
        }
        pose[HIPS].expect("keyed").translation
    };
    assert!(
        close(at(&anim), Vec3::new(1.0, 0.25, 0.5)),
        "off: the pose travels"
    );
    anim.apply_root_motion = true;
    assert!(
        close(at(&anim), Vec3::new(0.0, 0.25, 0.0)),
        "on: only the height"
    );
}

#[test]
fn a_bone_the_skeleton_lacks_measures_nothing() {
    let mut anim = playing("Lunge", false);
    anim.root_bone = "tail".to_string();
    assert!(step(&mut anim, Some(&lunge()), None, DT).root.is_none());
    anim.root_bone = "hips".to_string();
    assert!(step(&mut anim, Some(&lunge()), None, DT).root.is_some());
}
