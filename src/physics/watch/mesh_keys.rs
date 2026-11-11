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

use crate::components::ColliderShape;
use crate::core::collections::Map;
use crate::physics::shape::build::collider_inputs;
use crate::scene::mesh_library::{kept_mesh_name, scene_mesh_name};
use crate::scene::Scene;

/// Collider entity -> the fingerprint of the library-mesh triangles its shape was
/// built from. Only colliders built from a library mesh are listed.
pub(in crate::physics) type MeshKeys = Map<u32, u64>;

/// The fingerprint of the library-mesh triangles collider `id` builds from now, or
/// `None` when it builds from no library mesh (another shape, a collision mesh, a
/// model's or a primitive's Mesh).
pub(in crate::physics) fn key_of(scene: &Scene, id: u32) -> Option<u64> {
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
pub(in crate::physics) fn record(keys: &mut MeshKeys, scene: &Scene, ids: &[u32]) {
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
