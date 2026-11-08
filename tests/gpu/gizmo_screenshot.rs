//! A game screenshot with its gizmos (#1882): `Harness.Screenshot(path, { gizmos =
//! true })` draws the colliders and the `Debug.Draw*` lines over the frame, and
//! without the option the frame is byte-for-byte the plain render.

use glam::Vec3;
use rusty::dev::harness::Harness;
use rusty::scene::{ColliderComponent, ColliderShape};

#[test]
fn gizmos_draw_over_a_game_shot_and_leave_a_plain_one_alone() {
    let out = crate::temp::dir().join(format!("rusty_gizmo_gpu_{}", std::process::id()));
    let mut h = Harness::new(&out, "");
    h.set_size(320, 180);
    h.step(2);
    {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        let id = scene.add_entity("Capsule".to_string());
        scene.world.transform_mut(id).expect("fresh").position = Vec3::new(0.0, 1.5, -9.5);
        let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        col.shape = ColliderShape::Capsule {
            radius: 0.5,
            height: 2.0,
            axis: rusty::components::CapsuleAxis::Y,
        };
        scene.world.set_collider(id, Some(col));
        let red = [1.0, 0.0, 0.0, 1.0];
        let from = Vec3::new(-2.5, 2.0, -13.5);
        scene
            .debug_lines
            .push(from, Vec3::new(-0.5, 1.5, -9.5), red, 1);
    }
    let shot = |name: &str| out.join(name);
    if !h.screenshot(shot("plain.png")) {
        assert!(
            !rusty::render::gpu_required(),
            "RUSTY_REQUIRE_GPU=1 but no adapter"
        );
        return;
    }
    assert!(h.screenshot_with(shot("gizmos.png"), (320, 180), true));
    assert!(h.screenshot(shot("plain_again.png")));
    let read = |name: &str| image::open(shot(name)).expect("decodes").to_rgba8();
    let (plain, gizmos) = (read("plain.png"), read("gizmos.png"));
    assert_eq!(plain, read("plain_again.png"), "no option, no change");
    let changed = plain.pixels().zip(gizmos.pixels()).filter(|(a, b)| a != b);
    let lines: Vec<_> = changed.map(|(_, b)| b.0).collect();
    assert!(lines.len() > 50, "the gizmos drew: {} pixels", lines.len());
    let greenish = |p: &[u8; 4]| p[1] > p[0] && p[1] > p[2];
    assert!(lines.iter().any(greenish), "a collider's green");
    assert!(
        lines.iter().any(|p| p[0] > 200 && p[1] < 80),
        "the debug line's red"
    );
    let _ = std::fs::remove_dir_all(out);
}
