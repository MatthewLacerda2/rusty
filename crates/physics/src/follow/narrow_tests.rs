//! What reaches the kept bone list (#2345): edits away from the rigs, a busy
//! firefight's spawns and destroys among them, walk no skeleton; a hung mesh
//! collider rebuilt from new triangles joins and leaves the list without one.

use super::cost_tests::{collider, rig, tick};
use super::edit_ops::{define_plank, give_plank};
use crate::compound::world_pose;
use crate::convert::to_pose;
use crate::PhysicsWorld;
use rusty_components::RigidBodyComponent;
use rusty_scene::{mesh_library, Scene};

fn walks(physics: &PhysicsWorld) -> u64 {
    physics.tracked.follow.as_ref().map_or(0, |k| k.walks)
}

fn kept(physics: &PhysicsWorld) -> &[u32] {
    physics.tracked.follow.as_ref().unwrap().ids()
}

fn kinematic() -> RigidBodyComponent {
    RigidBodyComponent {
        is_kinematic: true,
        ..rusty_scene::create::defaults::default_rigidbody()
    }
}

#[test]
fn edits_away_from_the_rigs_walk_no_skeleton() {
    let mut scene = Scene::new();
    rig(&mut scene, true);
    let mut physics = PhysicsWorld::from_scene(&scene);
    tick(&mut physics, &mut scene);
    let prop = scene.add_entity("Prop".to_string());
    scene.world.set_collider(prop, Some(collider()));
    let decal = mesh_library::resolve_shared(&scene, "Box").unwrap();
    mesh_library::put_shared(&mut scene, prop, decal);
    tick(&mut physics, &mut scene);
    let shell = scene.add_entity("Shell".to_string());
    scene.world.set_collider(shell, Some(collider()));
    scene.world.set_rigidbody(shell, Some(kinematic()));
    tick(&mut physics, &mut scene);
    scene.set_parent(prop, Some(shell)).unwrap();
    tick(&mut physics, &mut scene);
    scene.world.collider_mut(prop).unwrap().active = false;
    scene.world.set_active(shell, false);
    tick(&mut physics, &mut scene);
    scene.destroy_entity(shell);
    tick(&mut physics, &mut scene);
    assert_eq!(walks(&physics), 1, "the first follow only");
    assert_eq!(kept(&physics).len(), 3, "the three hitboxes");
}

#[test]
fn a_hung_mesh_collider_rebuilt_from_its_triangles_joins_and_leaves_the_list() {
    let mut scene = Scene::new();
    let rig = rig(&mut scene, true);
    let head = scene.find_bone(rig, "head").unwrap();
    let hitbox = scene.world.children(head)[0];
    // The hitbox carries a body, so the visor's shape is rebuilt on it, not replanned.
    scene.world.set_rigidbody(hitbox, Some(kinematic()));
    let visor = scene.add_entity("Visor".to_string());
    scene.set_parent(visor, Some(hitbox)).unwrap();
    define_plank(&mut scene, false);
    give_plank(&mut scene, visor);
    let mut physics = PhysicsWorld::from_scene(&scene);
    tick(&mut physics, &mut scene);
    let walked = walks(&physics);
    assert!(
        !kept(&physics).contains(&visor),
        "nothing builds from no triangles"
    );
    define_plank(&mut scene, true);
    tick(&mut physics, &mut scene);
    assert!(kept(&physics).contains(&visor), "built from the cube");
    let at = *physics.colliders[physics.id_to_collider[&visor]].position();
    let pose = world_pose(&scene, visor).unwrap();
    assert_eq!(at, to_pose(pose.pos, pose.rot), "on its bone");
    define_plank(&mut scene, false);
    tick(&mut physics, &mut scene);
    assert!(
        !kept(&physics).contains(&visor),
        "dropped with its triangles"
    );
    assert_eq!(
        walks(&physics),
        walked,
        "a rebuild filters, it does not walk"
    );
}
