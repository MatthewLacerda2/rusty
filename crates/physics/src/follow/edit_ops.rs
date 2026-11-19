//! The edits the kept bone list's property test makes to a rig (#2345): each one
//! a way the set of bone-hung colliders can change, and a pose write that must
//! not change it.

use glam::Quat;
use proptest::prelude::*;

use super::cost_tests::collider;
use rusty_components::RigidBodyComponent;
use rusty_scene::skeleton::rig::{chain_skin, reimport_to};
use rusty_scene::Scene;

#[derive(Clone, Debug)]
pub(super) enum Op {
    /// A collider spawned under the picked entity.
    Spawn(usize),
    /// The first picked entity re-parented under the second, or to the root.
    Reparent(usize, Option<usize>),
    Destroy(usize),
    /// A Collider attached to the picked entity, or the one there detached.
    Attach(usize),
    /// A bone slot of the rig's skeleton cleared, as a destroyed bone leaves it.
    Unslot(usize),
    /// A Collider's `active` flipped.
    Toggle(usize),
    /// A kinematic Rigidbody added, or the one there removed.
    Body(usize),
    /// The rig re-imported to a shorter chain, or back to the full one.
    Reimport(bool),
    /// A pose write, as the Animator's.
    Swing(usize, i8),
}

pub(super) fn op() -> impl Strategy<Value = Op> {
    let i = || 0usize..64;
    prop_oneof![
        i().prop_map(Op::Spawn),
        (i(), proptest::option::of(i())).prop_map(|(a, b)| Op::Reparent(a, b)),
        i().prop_map(Op::Destroy),
        i().prop_map(Op::Toggle),
        i().prop_map(Op::Attach),
        i().prop_map(Op::Unslot),
        i().prop_map(Op::Body),
        any::<bool>().prop_map(Op::Reimport),
        (i(), any::<i8>()).prop_map(|(a, t)| Op::Swing(a, t)),
        (i(), any::<i8>()).prop_map(|(a, t)| Op::Swing(a, t)),
    ]
}

fn pick(scene: &Scene, i: usize) -> u32 {
    let ids = scene.world.ids();
    ids[i % ids.len()]
}

/// Make `op` to `scene`, whose skinned rig is `rig`.
pub(super) fn apply(scene: &mut Scene, rig: u32, op: &Op) {
    match *op {
        Op::Spawn(_) | Op::Reparent(..) | Op::Destroy(_) => hierarchy(scene, rig, op),
        _ => components(scene, rig, op),
    }
}

/// The edits that move an entity in, out of or around the hierarchy.
fn hierarchy(scene: &mut Scene, rig: u32, op: &Op) {
    match *op {
        Op::Spawn(i) => {
            let parent = pick(scene, i);
            let id = scene.add_entity("Spawned".to_string());
            let _ = scene.set_parent(id, Some(parent));
            scene.world.set_collider(id, Some(collider()));
        }
        Op::Reparent(a, b) => {
            let (id, to) = (pick(scene, a), b.map(|b| pick(scene, b)));
            let _ = scene.set_parent(id, to);
        }
        Op::Destroy(i) => {
            let id = pick(scene, i);
            // The rig stays, so every case keeps a skeleton to walk.
            if !scene.world.self_and_descendants(id).any(|d| d == rig) {
                scene.destroy_entity(id);
            }
        }
        _ => {}
    }
}

/// The edits to one entity's components.
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
        _ => {}
    }
}
