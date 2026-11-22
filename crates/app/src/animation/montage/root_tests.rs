use super::super::test_rig::*;
use super::super::*;
use super::tests::{start, with_montages, DT};
use rusty_components::{Motion, RootMotion};

fn lunge_rig() -> MeshComponent {
    mesh(vec![
        slide("Idle", &[LEG], 0.0, 0.0, 1.0),
        slide("Lunge", &[HIPS, SPINE], 0.0, 2.0, 1.0),
    ])
}

const MONTAGES: &str = r#"[
    { "name": "Lunge", "clip": "Lunge", "blend_in": 0, "blend_out": 0 },
    { "name": "Swing", "clip": "Lunge", "slot": "UpperBody", "blend_in": 0, "blend_out": 0 }]"#;
const SLOTS: &str = r#"[{ "name": "UpperBody", "mask": ["spine"] }]"#;

fn travel(montage: &str, n: usize) -> RootMotion {
    let (m, g) = (lunge_rig(), with_montages(SLOTS, MONTAGES));
    let mut anim = animator(Motion::Clip("Idle"), None, 0.0);
    start(&mut anim, &g, &m, montage);
    (0..n).fold(RootMotion::ZERO, |acc, _| {
        let root = step(&mut anim, Some(&m), Some(&g), DT).root;
        acc.then(root.unwrap_or_default())
    })
}

#[test]
fn a_full_body_montage_moves_the_entity() {
    let moved = travel("Lunge", 10);
    assert!((moved.position.x - 1.0).abs() < 1e-3, "{moved:?}");
    assert!(moved.position.z.abs() < 1e-4 && moved.yaw.abs() < 1e-4);
}

#[test]
fn a_montage_whose_slot_misses_the_root_bone_moves_nothing() {
    assert_eq!(travel("Swing", 10).position, glam::Vec3::ZERO);
}

#[test]
fn two_runs_of_the_same_inputs_pose_identically() {
    let pose_run = || {
        let (m, g) = (lunge_rig(), with_montages(SLOTS, MONTAGES));
        let mut anim = animator(Motion::Clip("Idle"), None, 0.0);
        start(&mut anim, &g, &m, "Swing");
        let mut poses = Vec::new();
        for i in 0..30 {
            if i == 7 {
                start(&mut anim, &g, &m, "Lunge");
            }
            step(&mut anim, Some(&m), Some(&g), DT);
            let pose = sample_pose(&anim, &m, Some(&g)).unwrap().1;
            poses.extend(pose.into_iter().flatten().map(|j| j.translation.to_array()));
        }
        poses
    };
    let (a, b) = (pose_run(), pose_run());
    let bits = |p: &[[f32; 3]]| p.iter().flatten().map(|f| f.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a), bits(&b));
}

#[test]
fn root_motion_stacks_in_the_slot_order_the_pose_does() {
    let m = mesh(vec![
        slide("Idle", &[LEG], 0.0, 0.0, 1.0),
        slide("Lunge", &[HIPS, SPINE], 0.0, 2.0, 1.0),
        slide("Back", &[HIPS, SPINE], 0.0, -1.0, 1.0),
    ]);
    let g = with_montages(
        r#"[{ "name": "FullBody" }]"#,
        r#"[{ "name": "Lunge", "clip": "Lunge", "slot": "FullBody", "blend_in": 0, "blend_out": 0 },
            { "name": "Back", "clip": "Back", "blend_in": 0, "blend_out": 0 }]"#,
    );
    let mut anim = animator(Motion::Clip("Idle"), None, 0.0);
    // `Back`'s implicit `DefaultSlot` (order 1) stacks above `FullBody` (0), though started first.
    start(&mut anim, &g, &m, "Back");
    start(&mut anim, &g, &m, "Lunge");
    let moved = (0..10).fold(RootMotion::ZERO, |acc, _| {
        acc.then(
            step(&mut anim, Some(&m), Some(&g), DT)
                .root
                .unwrap_or_default(),
        )
    });
    // 10 steps of 0.05 s: half of Back's -1 m, on top in the pose and the root motion alike.
    let hips = sample_pose(&anim, &m, Some(&g)).unwrap().1[HIPS].unwrap();
    assert!((hips.translation.x + 0.5).abs() < 1e-3, "{hips:?}");
    assert!((moved.position.x + 0.5).abs() < 1e-3, "{moved:?}");
}
