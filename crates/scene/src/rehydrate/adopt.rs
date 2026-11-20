//! A restore that takes geometry back from the World it replaces, instead of
//! rebuilding it from its reference.
//!
//! A document recorded without geometry (an Undo step, a Play snapshot, a prefab
//! baseline) comes back over a live World that usually still draws the same
//! meshes. Where the live mesh is the recorded one, its geometry moves onto the
//! document and nothing is copied, imported or uploaded again; where it is not, the
//! mesh is rebuilt as a load rebuilds it. Undo keeps a mesh from the same source
//! ([`Keep::SameSource`], `scene::undo::restore`): it runs on an edit frame, whose
//! model follow catches a write. Stop and the prefab propagation after it ask for
//! the same revision too, current against the import cache ([`Keep::Current`]):
//! Play can re-mesh an entity while keeping its reference, and a model can change
//! on disk during Play with nobody watching (a headless or MCP session).

use super::{rehydrate_in, Imports, RehydratedScene, Which};
use rusty_components::{ColliderComponent, ColliderShape, Entity, MeshComponent, SharedMesh};
use rusty_ecs::World;

/// Which live mesh a restore keeps the geometry of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// One from the same primitive or asset ([`MeshComponent::same_source`]).
    SameSource,
    /// One from the same source, at the recorded revision, which is the one its
    /// source builds now ([`Imports::current`]); a collider's own mesh, which has
    /// no revision, from a file the pass confirmed ([`Imports::confirmed`]).
    Current,
}

impl Keep {
    fn mesh(self, doc: &MeshComponent, live: &MeshComponent, imports: &mut Imports) -> bool {
        doc.same_source(live)
            && match self {
                Keep::SameSource => true,
                Keep::Current => doc.revision == live.revision && imports.current(live),
            }
    }

    fn shared(self, doc: &SharedMesh, live: &SharedMesh, imports: &Imports) -> bool {
        doc.same_source(live)
            && match self {
                Keep::SameSource => true,
                Keep::Current => imports.confirmed(live),
            }
    }
}

impl RehydratedScene {
    /// [`RehydratedScene::new`] for a Play snapshot restored over `live`, the World
    /// Play leaves, which the restore then clears. Each mesh Play left as it was
    /// ([`Keep::Current`]) takes `live`'s geometry, moved out; every other mesh is
    /// rebuilt from its reference. Each file is looked up once, and the lookups are
    /// kept for the prefab propagation that follows (`Self::into_parts`).
    /// Stamped before the lookups, so a model written while they ran is still
    /// followed after.
    pub fn adopting(mut data: crate::serialize::SceneData, live: &mut World) -> Self {
        let read_at = rusty_core::fs::written::written_since(u64::MAX).0;
        let mut imports = Imports::default();
        for doc in &mut data.entities {
            if !adopt_or_rehydrate(live, doc, Keep::Current, &mut imports) {
                unbind(doc);
            }
        }
        Self {
            data,
            read_at,
            imports,
        }
    }
}

/// Leave `doc`'s adopted Mesh unbound and at rest, as a rebuilt one is: for a
/// restore whose skeleton sync binds rebuilt bones (Stop, a prefab rebuild), not
/// for Undo's, which keeps the live bones.
pub fn unbind(doc: &mut Entity) {
    if let Some(mesh) = &mut doc.mesh {
        mesh.pose_palette.clear();
        mesh.skeleton.bones.clear();
    }
}

/// Give `doc` (recorded without geometry) its live entity's geometry where `keep`
/// holds ([`take_live`]), and rebuild it from its reference otherwise. Whether its
/// Mesh was rebuilt, so the caller binds its skeleton.
pub fn adopt_or_rehydrate(
    world: &mut World,
    doc: &mut Entity,
    keep: Keep,
    imports: &mut Imports,
) -> bool {
    if !take_live(world, doc, keep, imports) {
        // Rebuilds the collider's meshes too.
        rehydrate_in(doc, imports);
        return true;
    }
    false
}

/// Give `doc` (recorded without geometry) its live entity's mesh geometry when
/// `keep` holds for the two, and its collider's own meshes by
/// [`restore_collider_meshes`]. `false` when `doc`'s Mesh must be rebuilt instead
/// (with its collider, by `rehydrate_entity_mesh`).
pub fn take_live(world: &mut World, doc: &mut Entity, keep: Keep, imports: &mut Imports) -> bool {
    if let Some(mesh) = &mut doc.mesh {
        let Some(mut live) = world.mesh_mut(doc.id) else {
            return false;
        };
        if !keep.mesh(mesh, &live, imports) {
            return false;
        }
        mesh.adopt_geometry(&mut live);
    }
    restore_collider_meshes(world, doc, keep, imports);
    true
}

/// Put back `doc`'s collider's own meshes, recorded without geometry: the kept copy
/// when it has no Mesh (#1657), and the collision mesh (#1960) whatever it has. Both
/// are restored before the box is refit, once, to what it builds from, as
/// `rehydrate_entity_mesh` does: refitting after the first would size it to a kept
/// copy while the collision mesh still had no geometry.
fn restore_collider_meshes(world: &mut World, doc: &mut Entity, keep: Keep, imports: &mut Imports) {
    let own = doc
        .mesh
        .as_ref()
        .map(|m| m.vertices.iter().map(|v| v.position));
    let own = own.and_then(super::bounds_of);
    let mut rebuilt = false;
    if doc.mesh.is_none() {
        rebuilt |= restore_collider_mesh(world, doc, Which::Kept, keep, imports);
    }
    rebuilt |= restore_collider_mesh(world, doc, Which::Collision, keep, imports);
    if let Some(collider) = doc.collider.as_mut().filter(|_| rebuilt) {
        super::refit_collider(collider, own);
    }
}

/// Give `doc`'s collider's `which` mesh (recorded without geometry) the live one's
/// when `keep` holds, and rebuild it from its reference otherwise.
/// Only a Mesh-shaped collider's: another shape never reads it. Whether it was
/// rebuilt, so the caller refits the box.
fn restore_collider_mesh(
    world: &mut World,
    doc: &mut Entity,
    which: Which,
    keep: Keep,
    imports: &mut Imports,
) -> bool {
    let mesh_shaped = |c: &&mut ColliderComponent| matches!(c.shape, ColliderShape::Mesh { .. });
    let Some(collider) = doc.collider.as_mut().filter(mesh_shaped) else {
        return false;
    };
    let Some(shared) = which.of(collider).as_mut() else {
        return false;
    };
    if let Some(mut live) = world.collider_mut(doc.id) {
        if let Some(live) = which
            .of(&mut live)
            .as_mut()
            .filter(|l| keep.shared(shared, l, imports))
        {
            shared.adopt_geometry(live);
            return false;
        }
    }
    super::collider::rebuild(doc.id, collider, which, imports);
    true
}
