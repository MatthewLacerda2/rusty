//! Per-bone hitboxes end to end (#464): a ray names the bone it struck on a posed
//! character, a shot fired in `Update` hits the arm where the animation put it
//! that frame, and a CharacterController walks through hitboxes.

pub mod rig;

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use rusty::app::GameWorld;
use rusty::components::CharacterControllerComponent;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::physics::{self, PhysicsWorld};
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

/// The bone a +Z ray at height `y`, offset `x` from the rig, strikes.
fn bone_hit(scene: &Scene, physics: &PhysicsWorld, x: f32, y: f32) -> Option<(u32, u32)> {
    let hit = physics.raycast_hit(Vec3::new(x, y, -5.0), Vec3::Z, 20.0, |_| true)?;
    Some((scene.hit_bone(hit.id)?, scene.root_of(hit.id)))
}

#[test]
fn a_ray_names_the_bone_it_hits_on_a_posed_character() {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    let (head, spine) = (
        scene.find_bone(rig, "head").unwrap(),
        scene.find_bone(rig, "spine").unwrap(),
    );
    let physics = PhysicsWorld::from_scene(&scene);
    assert_eq!(
        bone_hit(&scene, &physics, 0.0, 1.85),
        Some((head, rig)),
        "at rest: the head"
    );
    assert_eq!(
        bone_hit(&scene, &physics, 0.0, 1.35),
        Some((spine, rig)),
        "and the chest"
    );
    // Bend the spine 90° to the side: the head now sits at x 0.65, y 1.2.
    scene.world.transform_mut(spine).unwrap().rotation =
        Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    let physics = PhysicsWorld::from_scene(&scene);
    assert_eq!(
        bone_hit(&scene, &physics, 0.65, 1.2),
        Some((head, rig)),
        "posed: the head moved"
    );
    assert_eq!(
        bone_hit(&scene, &physics, 0.0, 1.85),
        None,
        "nothing where it was"
    );
}

/// Each `Update`, fire straight down onto where the arm's hitbox is this frame
/// and tally `Sink`'s x for a hit naming the arm of this rig, y for anything else.
const SHOOTER: &str = "local hits, misses = 0, 0\n\
return { Update = function(id)\n\
  local _, _, z = Transform.GetPosition(Animator.GetBone(id, 'arm'))\n\
  local mask = 1 << Layers.NameToIndex('Hitbox')\n\
  local hit, _, _, _, _, _, _, _, _, bone, name, root = Physics.Raycast(0.445, 3, z, 0, -1, 0, nil, mask)\n\
  if hit and bone and name == 'arm' and root == id then hits = hits + 1 else misses = misses + 1 end\n\
  Transform.SetPosition(Scene.FindEntityByName('Sink'), hits, misses, 0)\n\
end }";

#[test]
fn a_shot_in_update_hits_the_arm_where_the_animation_put_it() {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, true);
    scene.add_entity("Sink".to_string());
    let path = crate::temp::dir().join("rusty_464_shooter.lua");
    std::fs::write(&path, SHOOTER).unwrap();
    *scene.world.scripts_mut(rig).unwrap() = vec![ScriptComponent {
        path: path.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
    let scene = Rc::new(RefCell::new(scene));
    let nav = NavigationGraph::new(-5.0, 5.0, -5.0, 5.0, 1.0);
    let mut gw = GameWorld::new(
        scene.clone(),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    gw.set_playing(true);
    for _ in 0..40 {
        gw.tick(1.0 / 60.0); // the arm moves 0.1 a tick, twice its hitbox radius
    }
    let s = scene.borrow();
    let tally = s
        .world
        .transform(s.find_entity_by_name("Sink").unwrap())
        .unwrap()
        .position;
    let arm_z = s
        .world
        .transform(s.find_bone(rig, "arm").unwrap())
        .unwrap()
        .position
        .z;
    assert!(arm_z > 3.0, "the arm swung, z = {arm_z}");
    assert!(
        tally.x >= 38.0 && tally.y == 0.0,
        "every shot hit the moving arm: {tally:?}"
    );
}

/// Walk a CharacterController +X straight through the rig's torso; returns where
/// it ended.
fn walk_through(scene: &mut Scene) -> f32 {
    let cc = scene.add_entity("Walker".to_string());
    scene.world.transform_mut(cc).unwrap().position = Vec3::new(-2.0, 1.1, 0.0);
    scene
        .world
        .set_character_controller(cc, Some(CharacterControllerComponent::default()));
    let mut world = PhysicsWorld::from_scene(scene);
    for _ in 0..40 {
        physics::move_character(Some(&world), scene, cc, Vec3::X * 0.1);
        world.step(scene, 1.0 / 60.0);
    }
    scene.world.transform(cc).unwrap().position.x
}

#[test]
fn a_character_controller_walks_through_hitboxes() {
    let mut scene = Scene::new();
    rig::character(&mut scene, Vec3::ZERO, false);
    let x = walk_through(&mut scene);
    assert!((x - 2.0).abs() < 1e-3, "nothing blocked the walk: x = {x}");
    // Control: let the hitbox layer collide with Default and the torso blocks it.
    let mut scene = Scene::new();
    rig::character(&mut scene, Vec3::ZERO, false);
    let layer = scene.layers.index_of("Hitbox").unwrap();
    scene.collision_matrix.set_collision(0, layer, true);
    assert!(
        walk_through(&mut scene) < 0.0,
        "a colliding hitbox would block it"
    );
}
