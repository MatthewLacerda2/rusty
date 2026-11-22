use glam::{Quat, Vec3};
use proptest::prelude::*;

use super::cost_tests::collider;
use rusty_components::mesh::MeshData;
use rusty_components::{ColliderComponent, RigidBodyComponent, SharedMesh};
use rusty_scene::mesh_library::{kept_mesh_name, resolve_shared};
use rusty_scene::skeleton::rig::{chain_skin, reimport_to};
use rusty_scene::Scene;

#[derive(Clone, Debug)]
pub(super) enum Op {
    Spawn(Option<usize>),
    Reparent(usize, Option<usize>),
    Destroy(usize),
    Attach(usize),
    Unslot(usize),
    Toggle(usize),
    SetActive(Option<usize>, bool),
    Resync,
    Body(usize),
    Reimport(bool),
    Swing(usize, i8),
    Plank(usize, bool),
}

pub(super) fn op() -> impl Strategy<Value = Op> {
    let i = || 0usize..64;
    prop_oneof![
        proptest::option::of(i()).prop_map(Op::Spawn),
        proptest::option::of(i()).prop_map(Op::Spawn),
        (i(), proptest::option::of(i())).prop_map(|(a, b)| Op::Reparent(a, b)),
        i().prop_map(Op::Destroy),
        i().prop_map(Op::Toggle),
        (proptest::option::of(i()), any::<bool>()).prop_map(|(a, on)| Op::SetActive(a, on)),
        Just(Op::Resync),
        i().prop_map(Op::Attach),
        i().prop_map(Op::Unslot),
        i().prop_map(Op::Body),
        any::<bool>().prop_map(Op::Reimport),
        (i(), any::<i8>()).prop_map(|(a, t)| Op::Swing(a, t)),
        (i(), any::<i8>()).prop_map(|(a, t)| Op::Swing(a, t)),
        (i(), any::<bool>()).prop_map(|(a, v)| Op::Plank(a, v)),
    ]
}

fn pick(scene: &Scene, i: usize) -> u32 {
    let ids = scene.world.ids();
    ids[i % ids.len()]
}

pub(super) fn apply(scene: &mut Scene, rig: u32, op: &Op) {
    match *op {
        Op::Spawn(_) | Op::Reparent(..) | Op::Destroy(_) | Op::SetActive(..) | Op::Resync => {
            hierarchy(scene, rig, op)
        }
        _ => components(scene, rig, op),
    }
}

fn hierarchy(scene: &mut Scene, rig: u32, op: &Op) {
    match *op {
        Op::Spawn(i) => {
            let parent = i.map(|i| pick(scene, i));
            let id = scene.add_entity("Spawned".to_string());
            let _ = scene.set_parent(id, parent);
            scene.world.set_collider(id, Some(collider()));
        }
        Op::Reparent(a, b) => {
            let (id, to) = (pick(scene, a), b.map(|b| pick(scene, b)));
            let _ = scene.set_parent(id, to);
        }
        Op::Destroy(i) => {
            let id = pick(scene, i);
            if !scene.world.self_and_descendants(id).any(|d| d == rig) {
                scene.destroy_entity(id);
            }
        }
        Op::SetActive(i, on) => {
            let id = i.map_or(rig, |i| pick(scene, i));
            scene.world.set_active(id, on);
        }
        Op::Resync => scene.hang_on_skeletons(Vec::new(), Vec::new()),
        _ => {}
    }
}

fn plank(scene: &mut Scene, rig: u32, i: usize, cube: bool) {
    define_plank(scene, cube);
    let id = pick(scene, i);
    let w = &scene.world;
    let built = w
        .collider(id)
        .is_some_and(|c| kept_mesh_name(&c) == Some("plank"));
    if id != rig && !built && w.mesh(id).is_none() {
        give_plank(scene, id);
    }
}

pub(super) fn define_plank(scene: &mut Scene, cube: bool) {
    let data = if cube {
        unit_cube()
    } else {
        MeshData::default()
    };
    scene.define_mesh("plank", data).unwrap();
}

pub(super) fn give_plank(scene: &mut Scene, id: u32) {
    let mesh = resolve_shared(scene, "plank").unwrap();
    let shape = ColliderComponent {
        shared_mesh: Some(SharedMesh::of(&mesh)),
        ..ColliderComponent::from_mesh_bounds(true, Vec3::ZERO, Vec3::ZERO)
    };
    scene.world.set_collider(id, Some(shape));
}

fn unit_cube() -> MeshData {
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
        .flat_map(|[a, b, c, d]| [*a, *b, *c, *a, *c, *d]);
    MeshData {
        positions,
        indices: indices.collect(),
        ..Default::default()
    }
}

fn components(scene: &mut Scene, rig: u32, op: &Op) {
    match *op {
        Op::Toggle(i) => {
            if let Some(mut c) = scene.world.collider_mut(pick(scene, i)) {
                c.active = !c.active;
            }
        }
        Op::Attach(i) => {
            let id = pick(scene, i);
            let c = (!scene.world.has_collider(id)).then(collider);
            scene.world.set_collider(id, c);
        }
        Op::Unslot(k) => {
            if let Some(mut mesh) = scene.world.mesh_mut(rig) {
                let bones = &mut mesh.skeleton.bones;
                let n = bones.len().max(1);
                if let Some(slot) = bones.get_mut(k % n) {
                    *slot = None;
                }
            }
        }
        Op::Body(i) => {
            let id = pick(scene, i);
            let body = (!scene.world.has_rigidbody(id)).then(|| RigidBodyComponent {
                is_kinematic: true,
                ..rusty_scene::create::defaults::default_rigidbody()
            });
            scene.world.set_rigidbody(id, body);
        }
        Op::Reimport(short) => {
            let names: &[&str] = if short {
                &["hips", "spine"]
            } else {
                &["hips", "spine", "head"]
            };
            if scene.world.mesh(rig).is_some() {
                reimport_to(scene, rig, chain_skin(names));
            }
        }
        Op::Swing(i, t) => {
            if let Some(mut tr) = scene.world.transform_mut(pick(scene, i)) {
                tr.rotation = Quat::from_rotation_z(f32::from(t) * 0.02);
            }
        }
        Op::Plank(i, cube) => plank(scene, rig, i, cube),
        _ => {}
    }
}
