//! src/app/systems/animate_tests.rs — root motion through the real play loop
//! (#1745): a lunge whose hips move 2 m moves the entity 2 m with the hips held under it, a wall stops a
//! CharacterController's lunge, and a NavMeshAgent follows the lunge instead of
//! its own speed.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::animation::test_rig::{mesh, slide, HIPS};
use crate::app::game::game_tests::{world_with, DT};
use crate::components::{
    AnimatorComponent, CharacterControllerComponent, ColliderComponent, ColliderShape,
    NavMeshAgentComponent, Playback,
};
use crate::scene::Scene;

/// A rig entity playing the lunge (hips +2 m along +X over 1 s, once), its
/// skeleton spawned, with root motion applied.
fn lunging(scene: &mut Scene) -> u32 {
    let id = scene.add_entity("Guard".to_string());
    let rig = mesh(vec![slide("Lunge", &[HIPS], 0.0, 2.0, 1.0)]);
    scene.world.set_mesh(id, Some(rig));
    scene.sync_skeletons();
    let anim = AnimatorComponent {
        base: Playback {
            current_clip: "Lunge".to_string(),
            ..Playback::default()
        },
        is_playing: true,
        apply_root_motion: true,
        ..AnimatorComponent::default()
    };
    scene.world.set_animator(id, Some(anim));
    id
}

/// A static box of `size` centred on `at`.
fn add_box(scene: &mut Scene, at: Vec3, size: Vec3) {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = at;
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size },
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
}

/// Play `scene` for 1.5 s; the guard's world position each tick, and the
/// hips' world position each tick.
fn play(scene: Scene, guard: u32) -> Vec<(Vec3, Vec3)> {
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    (0..90)
        .map(|_| {
            gw.tick(DT);
            let s = scene.borrow();
            let hips = s.find_bone(guard, "hips").expect("the hips bone");
            (s.world.world_position(guard), s.world.world_position(hips))
        })
        .collect()
}

#[test]
fn a_lunge_moves_the_entity_two_metres_and_the_hips_stay_under_it() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    let trace = play(scene, guard);
    let (end, _) = trace[trace.len() - 1];
    assert!(end.distance(Vec3::X * 2.0) < 1e-3, "moved 2 m: {end}");
    for (body, hips) in &trace {
        assert!((hips.x - body.x).abs() < 1e-4, "no slide: {body} vs {hips}");
    }
    let anim = trace[30].0.x - trace[29].0.x;
    assert!(
        (anim - 2.0 * DT).abs() < 1e-3,
        "a steady 2 m/s mid-lunge: {anim}"
    );
}

#[test]
fn a_wall_stops_a_character_controllers_lunge() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    let cc = CharacterControllerComponent::default();
    scene.world.set_character_controller(guard, Some(cc));
    add_box(&mut scene, Vec3::X * 1.5, Vec3::new(0.2, 4.0, 4.0));
    let (end, hips) = play(scene, guard)[89];
    assert!(
        end.x > 0.5 && end.x < 1.0,
        "stopped short of the wall: {end}"
    );
    assert!((hips.x - end.x).abs() < 1e-4, "the hips stay with the body");
}

#[test]
fn a_nav_agent_follows_the_lunge_not_its_speed() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::Y * -0.5, Vec3::new(20.0, 1.0, 20.0));
    let guard = lunging(&mut scene);
    let agent = NavMeshAgentComponent {
        active: true,
        radius: 0.3,
        speed: 5.0,
        acceleration: 50.0,
        target: Vec3::new(-8.0, 0.0, 0.0),
        update_rotation: false,
        ..NavMeshAgentComponent::default()
    };
    scene.world.set_nav_agent(guard, Some(agent));
    let (end, _) = play(scene, guard)[89];
    assert!(
        (end.x - 2.0).abs() < 1e-2,
        "the lunge moved it, not its speed: {end}"
    );
    assert!(end.y.abs() < 1e-3, "on the floor: {end}");
}

#[test]
fn scripts_read_the_delta_and_turn_root_motion_off() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT);
    let eval = |gw: &crate::app::GameWorld, line: &str| gw.script_manager().eval(line).unwrap();
    let dx: f32 = eval(&gw, &format!("return (Animator.GetRootMotion({guard}))"))
        .parse()
        .unwrap();
    assert!((dx - 2.0 * DT).abs() < 1e-4, "{dx}");
    eval(&gw, &format!("Animator.SetApplyRootMotion({guard}, false)"));
    let before = scene.borrow().world.world_position(guard);
    gw.tick(DT);
    assert_eq!(
        scene.borrow().world.world_position(guard),
        before,
        "off: it stays"
    );
    let still: f32 = eval(&gw, &format!("return (Animator.GetRootMotion({guard}))"))
        .parse()
        .unwrap();
    assert!(still > 0.0, "still measured while off");
}
