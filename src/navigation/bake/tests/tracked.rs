//! The play loop's sync (#1707, `bake::still`): `sync_changes`, which skips the
//! static colliders when the change log names no static entity, gives the navmesh
//! a plain `sync` gives after any sequence of World edits; and a dynamic entity
//! moving leaves the static geometry still.

use glam::Vec3;
use proptest::prelude::*;

use super::super::super::test_support::{add_box, add_box_with, add_floor};
use super::super::super::{NavBounds, NavigationGraph};
use crate::components::ColliderShape;
use crate::ecs::changes::ops_tests::{self, Edit};
use crate::scene::Scene;

#[derive(Clone, Debug)]
enum Op {
    World(ops_tests::Op),
    /// Give the entity at this index a box collider, or a trigger one.
    Collider(usize, bool),
    NoCollider(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => ops_tests::op().prop_map(Op::World),
        2 => (0usize..10, any::<bool>()).prop_map(|(i, t)| Op::Collider(i, t)),
        1 => (0usize..10).prop_map(Op::NoCollider),
    ]
}

/// A floor, three static crates and a dynamic one, on a grid sized by the geometry.
fn level() -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    add_floor(&mut scene, 0.0, 6.0, 0.0, 6.0);
    for i in 0..3 {
        let min = Vec3::new(1.0 + i as f32 * 1.5, 0.0, 1.0);
        add_box(&mut scene, min, min + Vec3::new(1.0, 1.0, 1.0));
    }
    add_box_with(
        &mut scene,
        Vec3::new(1.0, 0.0, 4.0),
        Vec3::new(2.0, 1.0, 5.0),
        false,
    );
    scene
}

fn apply(scene: &mut Scene, op: &Op) {
    let pick = |s: &Scene, i: usize| s.world.ids().get(i).copied();
    match op {
        // An Undo restores the parent's record and rewinds the log too
        // (`scene::undo::restore::apply`); the bare restore is physics' test's.
        Op::World(ops_tests::Op::On(_, Edit::UndoFlip | Edit::UndoDespawn)) => {}
        Op::World(op) => ops_tests::apply(&mut scene.world, op),
        Op::Collider(i, trigger) => {
            if let Some(e) = pick(scene, *i) {
                let c = crate::components::ColliderComponent {
                    shape: ColliderShape::Box { size: Vec3::ONE },
                    is_trigger: *trigger,
                    ..crate::scene::authoring::default_collider()
                };
                scene.world.set_collider(e, Some(c));
            }
        }
        Op::NoCollider(i) => {
            if let Some(e) = pick(scene, *i) {
                scene.world.set_collider(e, None);
            }
        }
    }
}

fn same(a: &NavigationGraph, b: &NavigationGraph) -> bool {
    (a.bounds(), a.grid_spacing, &a.cell_start, &a.spans)
        == (b.bounds(), b.grid_spacing, &b.cell_start, &b.spans)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn sync_changes_equals_sync(ops in proptest::collection::vec(op(), 1..40)) {
        let mut scene = level();
        let mut fast = NavigationGraph::from_scene(&scene);
        let mut full = NavigationGraph::from_scene(&scene);
        fast.bake_tracked(&mut scene);
        for (n, op) in ops.iter().enumerate() {
            apply(&mut scene, op);
            fast.sync_changes(&mut scene);
            full.sync(&scene);
            prop_assert!(same(&fast, &full), "the navmeshes part after op {}", n);
        }
    }
}

/// A dynamic crate moving, or nothing at all, leaves the static geometry still;
/// a static crate moving does not.
#[test]
fn only_a_static_change_reads_the_static_colliders() {
    let mut scene = level();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 6.0, 0.0, 6.0));
    let mut g = NavigationGraph::from_scene(&scene);
    g.bake_tracked(&mut scene);
    let ids = scene.world.ids().to_vec();
    let still = |s: &mut Scene, g: &mut NavigationGraph| {
        g.static_watch.as_mut().unwrap().still(&mut s.world)
    };
    assert!(still(&mut scene, &mut g), "nothing changed");
    scene.world.transform_mut(ids[4]).unwrap().position.x += 0.5;
    assert!(still(&mut scene, &mut g), "a dynamic crate moved");
    scene.world.transform_mut(ids[1]).unwrap().position.x += 0.5;
    assert!(!still(&mut scene, &mut g), "a static crate moved");
    assert!(still(&mut scene, &mut g), "and was read");
}
