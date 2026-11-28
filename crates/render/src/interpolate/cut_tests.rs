//! Speed never decides a cut: a pose moved at any speed blends, one stated set
//! (`World::cut_pose`) is drawn where it landed, and a subtree follows its root.

use glam::Vec3;

use super::*;
use rusty_scene::create::{create_entity, Primitive};

/// 400 m/s at 60 Hz.
const STEP: f32 = 400.0 / 60.0;

fn at(scene: &mut Scene, id: u32, x: f32) {
    scene.world.set_world_position(id, Vec3::new(x, 0.0, 0.0));
}

fn drawn_x(poses: &DrawPoses, scene: &Scene, id: u32) -> f32 {
    poses.world(scene, id).w_axis.x
}

/// A fast body with a child, recorded at rest and after one 400 m/s tick.
fn fast() -> (Scene, [u32; 2], DrawPoses) {
    let mut scene = Scene::new();
    let body = create_entity(&mut scene, "Missile", Some(Primitive::Box));
    let fin = create_entity(&mut scene, "Fin", Some(Primitive::Box));
    scene.set_parent(fin, Some(body)).unwrap();
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    at(&mut scene, body, STEP);
    poses.record(&scene);
    (scene, [body, fin], poses)
}

#[test]
fn a_body_at_400_m_s_and_its_child_are_drawn_between_two_ticks() {
    let (scene, [body, fin], mut poses) = fast();
    poses.set_alpha(0.5);
    assert!((drawn_x(&poses, &scene, body) - STEP / 2.0).abs() < 1e-4);
    assert!((drawn_x(&poses, &scene, fin) - STEP / 2.0).abs() < 1e-4);
}

#[test]
fn a_stated_cut_is_drawn_where_it_landed_with_its_subtree() {
    let (mut scene, [body, fin], mut poses) = fast();
    at(&mut scene, body, STEP + 5.0);
    scene.world.cut_pose(body);
    poses.record(&scene);
    for alpha in [0.0, 0.5, 0.99] {
        poses.set_alpha(alpha);
        for id in [body, fin] {
            let live = scene.world.local_to_world(id);
            assert_eq!(poses.world(&scene, id), live, "no in-between frame");
        }
    }
    at(&mut scene, body, 2.0 * STEP + 5.0);
    poses.record(&scene);
    poses.set_alpha(0.5);
    let x = drawn_x(&poses, &scene, body);
    assert!(
        (x - (1.5 * STEP + 5.0)).abs() < 1e-4,
        "the next tick blends again: {x}"
    );
}

#[test]
fn a_fast_body_is_drawn_at_an_even_step_across_tick_boundaries() {
    // What the motion vectors read: one frame's drawn pose against the last.
    let mut scene = Scene::new();
    let body = create_entity(&mut scene, "Jet", Some(Primitive::Box));
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    let (frame, mut clock, mut tick, mut last) = (1.0 / 144.0, 0.0_f32, 0, None);
    for _ in 0..40 {
        clock += frame;
        while clock >= (tick + 1) as f32 / 60.0 {
            tick += 1;
            at(&mut scene, body, tick as f32 * STEP);
            poses.record(&scene);
        }
        poses.set_alpha(clock * 60.0 - tick as f32);
        let x = drawn_x(&poses, &scene, body);
        if let (Some(before), true) = (last, tick >= 2) {
            let moved: f32 = x - before;
            assert!((moved - 400.0 * frame).abs() < 1e-2, "a step of {moved} m");
        }
        last = (tick >= 1).then_some(x);
    }
}
