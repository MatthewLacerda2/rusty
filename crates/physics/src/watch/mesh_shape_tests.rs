use super::mesh_keys_tests::{plank, scene, y, DT};
use crate::shape::fit::DECOMPOSED;
use crate::PhysicsWorld;
use rusty_components::mesh::MeshData;
use rusty_scene::{mesh_library, Scene};

fn floor(scene: &Scene) -> u32 {
    scene.mesh_users("plank")[0]
}

fn rest_then(always_full: bool, take: impl Fn(&mut Scene, u32)) -> (f32, bool) {
    let (mut scene, crate_) = scene();
    let floor = floor(&scene);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.tracked.always_full = always_full;
    for _ in 0..300 {
        world.step(&mut scene, DT);
    }
    assert!(
        (y(&scene, crate_) - 0.25).abs() < 0.05,
        "rests on the plank"
    );
    take(&mut scene, floor);
    for _ in 0..60 {
        world.step(&mut scene, DT);
    }
    (y(&scene, crate_), world.id_to_collider.contains_key(&floor))
}

#[test]
fn a_crate_falls_when_the_planks_mesh_is_deleted_and_comes_back_with_it() {
    for always_full in [false, true] {
        let (after, shaped) = rest_then(always_full, |s, _| {
            s.delete_mesh("plank", true).unwrap();
        });
        assert!(after < -1.0 && !shaped, "{after} ({always_full})");
    }
    let (mut scene, _) = scene();
    let floor = floor(&scene);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    scene.delete_mesh("plank", true).unwrap();
    world.step(&mut scene, DT);
    assert!(!world.id_to_collider.contains_key(&floor), "no shape");
    scene.define_mesh("plank", plank(-2.0, 2.0)).unwrap();
    let fresh = mesh_library::resolve_shared(&scene, "plank").unwrap();
    mesh_library::put_shared(&mut scene, floor, fresh);
    world.step(&mut scene, DT);
    assert!(world.id_to_collider.contains_key(&floor), "built again");
}

#[test]
fn a_crate_falls_when_the_plank_draws_a_primitive_instead() {
    for always_full in [false, true] {
        let (after, shaped) = rest_then(always_full, |s, floor| {
            let fresh = mesh_library::resolve_shared(s, "Box").unwrap();
            mesh_library::put_shared(s, floor, fresh);
        });
        assert!(after < -1.0 && shaped, "{after} ({always_full})");
    }
}

#[test]
fn a_crate_falls_when_the_plank_is_replaced_with_an_empty_mesh() {
    for always_full in [false, true] {
        let (after, shaped) = rest_then(always_full, |s, _| {
            s.define_mesh("plank", MeshData::default()).unwrap();
        });
        assert!(after < -1.0 && !shaped, "{after} ({always_full})");
    }
}

fn cube() -> MeshData {
    let positions = (0..8u8)
        .map(|i| [i & 1, i >> 1 & 1, i >> 2 & 1].map(|b| f32::from(b) - 0.5))
        .collect();
    let faces = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let indices = faces
        .iter()
        .flat_map(|[a, b, c, d]| [*a, *b, *c, *a, *c, *d])
        .collect();
    MeshData {
        positions,
        indices,
        ..Default::default()
    }
}

#[test]
fn a_mesh_replaced_as_its_body_turns_dynamic_gets_convex_pieces() {
    for always_full in [false, true] {
        let (mut scene, _) = scene();
        let floor = floor(&scene);
        let mut world = PhysicsWorld::from_scene(&scene);
        world.tracked.always_full = always_full;
        world.step(&mut scene, DT);
        scene.define_mesh("plank", cube()).unwrap();
        scene.world.set_static(floor, false);
        let rb = rusty_scene::create::defaults::default_rigidbody();
        let rb = rusty_components::RigidBodyComponent {
            is_kinematic: false,
            ..rb
        };
        scene.world.set_rigidbody(floor, Some(rb));
        world.step(&mut scene, DT);
        let c = &world.colliders[world.id_to_collider[&floor]];
        assert_eq!(c.user_data, DECOMPOSED, "{always_full}");
        assert!(
            c.shape().as_compound().is_some(),
            "convex pieces ({always_full})"
        );
    }
}
