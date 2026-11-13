//! Which mesh colliders' triangles changed since their shape was built (#2059).
//!
//! A mesh collider's triangles are read when its body is built, and a library
//! mesh (`Mesh.Create`) can be replaced in Play under a body that stays. So each
//! collider whose triangles come from one is remembered by a fingerprint of those
//! triangles, taken when its shape was built ([`record`]); the sync compares it
//! with the triangles it would build from now (`PhysicsWorld::sync_mesh_shapes`)
//! and rebuilds the shape where they moved. A change shows up on the Collider
//! (a kept copy, #1657) or the Mesh channel, so the incremental sync looks only at
//! the ids stamped there; the full pass looks at every collider.
//!
//! A collider whose recorded triangles are gone (its library mesh deleted, its
//! Mesh pointed at a primitive) is rebuilt from what its entity has now, and one
//! whose rebuild fails (an empty mesh, nothing left to build from) loses its
//! shape: its rapier collider is removed, as a fresh build from nothing makes
//! none, and comes back when its triangles do.

use std::collections::BTreeSet;

use crate::compound::world_pose;
use crate::shape::build::{class_of, collider_inputs, BodyClass};
use crate::watch::Owners;
use crate::world::{collider_to_write, PhysicsWorld};
use rusty_components::ColliderShape;
use rusty_core::collections::Map;
use rusty_scene::mesh_library::{kept_mesh_name, scene_mesh_name};
use rusty_scene::Scene;

/// Collider entity -> the fingerprint of the library-mesh triangles its shape was
/// built from. Only colliders built from a library mesh are listed.
pub(crate) type MeshKeys = Map<u32, u64>;

/// The fingerprint of the library-mesh triangles collider `id` builds from now, or
/// `None` when it builds from no library mesh (another shape, a collision mesh, a
/// model's or a primitive's Mesh).
pub(crate) fn key_of(scene: &Scene, id: u32) -> Option<u64> {
    let w = &scene.world;
    let collider = w.collider(id)?;
    if !matches!(collider.shape, ColliderShape::Mesh { .. })
        || collider.built_collision_mesh().is_some()
    {
        return None;
    }
    let library = match w.mesh(id) {
        Some(mesh) => scene_mesh_name(&mesh).is_some(),
        None => kept_mesh_name(&collider).is_some(),
    };
    drop(collider);
    if !library {
        return None;
    }
    let (positions, indices) = collider_inputs(w, id)?.mesh_geom?;
    Some(fingerprint(&positions, &indices))
}

/// Remember the triangles each of `ids` was just built from.
pub(crate) fn record(keys: &mut MeshKeys, scene: &Scene, ids: &[u32]) {
    for &id in ids {
        match key_of(scene, id) {
            Some(key) => keys.insert(id, key),
            None => keys.remove(&id),
        };
    }
}

/// FNV-1a over the positions' bits and the indices: equal triangles, equal key.
fn fingerprint(positions: &[[f32; 3]], indices: &[u32]) -> u64 {
    let words = positions.iter().flatten().map(|x| x.to_bits());
    let words = words.chain([u32::MAX]).chain(indices.iter().copied());
    words.fold(0xcbf2_9ce4_8422_2325, |hash, word| {
        (hash ^ u64::from(word)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl PhysicsWorld {
    /// Remember the triangles the colliders of the `built` owners were built from.
    pub(crate) fn record_mesh_keys(&mut self, scene: &Scene, built: &[u32]) {
        let plan = &self.plan;
        let ids: Vec<u32> = built
            .iter()
            .filter_map(|o| plan.get(o))
            .flatten()
            .copied()
            .collect();
        record(&mut self.tracked.mesh_keys, scene, &ids);
    }

    /// Rebuild each mesh collider whose library-mesh triangles differ from the ones
    /// its shape was built from: every collider in `only`, or every one when
    /// `None`, in plan order. See the module docs.
    pub(crate) fn sync_mesh_shapes(&mut self, scene: &Scene, only: Option<&BTreeSet<u32>>) {
        let owners = match only {
            None => Owners::All,
            Some(ids) => Owners::Only(
                (ids.iter())
                    .filter_map(|id| self.tracked.member_owner.get(id).copied())
                    .collect(),
            ),
        };
        let picked = owners
            .pick(&self.plan)
            .flat_map(|(&o, ids)| ids.iter().map(move |&id| (o, id)));
        let picked: Vec<(u32, u32)> = picked
            .filter(|(_, id)| only.is_none_or(|s| s.contains(id)))
            .collect();
        for (owner, id) in picked {
            let now = key_of(scene, id);
            let was = match now {
                Some(key) => self.tracked.mesh_keys.insert(id, key),
                None => self.tracked.mesh_keys.remove(&id),
            };
            if now != was {
                self.rebuild_mesh_shape(scene, owner, id);
            }
        }
    }

    /// Build collider `id` again from what its entity has now: its shape, hooks and
    /// fit stamp swapped onto the same rapier collider (so `sync_fits` after it
    /// finds the fit current), a collider that cannot be built removed, and
    /// one removed before attached again to its owner's body. An owner with no
    /// body (none of its colliders built) waits for the next replan.
    fn rebuild_mesh_shape(&mut self, scene: &Scene, owner: u32, id: u32) {
        let Some(&body) = self.id_to_body.get(&owner) else {
            return;
        };
        let dynamic = matches!(class_of(&scene.world, owner), BodyClass::Dynamic);
        let built = world_pose(scene, owner)
            .and_then(|pose| self.build_collider(scene, &pose, id, dynamic));
        match (built, self.id_to_collider.get(&id).copied()) {
            (Some(built), Some(handle)) => {
                if let Some(c) = collider_to_write(&mut self.colliders, handle, |_| true) {
                    c.set_shape(built.shared_shape().clone());
                    c.set_active_hooks(built.active_hooks());
                    c.user_data = built.user_data;
                }
            }
            (Some(built), None) => {
                let handle = self
                    .colliders
                    .insert_with_parent(built, body, &mut self.bodies);
                self.collider_to_id.insert(handle, id);
                self.id_to_collider.insert(id, handle);
            }
            (None, Some(handle)) => {
                self.colliders.remove(
                    handle,
                    &mut self.islands,
                    &mut self.bodies,
                    &mut self.soft_bodies,
                    true,
                );
                self.collider_to_id.remove(&handle);
                self.id_to_collider.remove(&id);
            }
            (None, None) => {}
        }
    }
}
