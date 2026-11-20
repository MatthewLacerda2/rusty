//! A restore that takes geometry back from the World it replaces, instead of
//! rebuilding it from its reference.
//!
//! A document recorded without geometry (an Undo step, a Play snapshot) comes
//! back over a live World that usually still draws the same meshes. Where the
//! live mesh is the recorded one, its geometry moves onto the document and
//! nothing is imported or uploaded again; where it is not, the mesh is rebuilt as
//! a load rebuilds it. Undo keeps a mesh from the same source
//! (`scene::undo::restore`); Stop asks for the same revision too, since Play can
//! re-mesh an entity while keeping its reference ([`RehydratedScene::adopting`]).

use super::{rehydrate_in, Imports, RehydratedScene, Which};
use rusty_components::{ColliderComponent, ColliderShape, Entity, MeshComponent};
use rusty_core::fs::written::{written_since, Stale};
use rusty_ecs::World;

impl RehydratedScene {
    /// [`RehydratedScene::new`] for a Play snapshot restored over `live`, the World
    /// Play leaves, which the restore then clears. `read_at` is the write-log
    /// generation the snapshot was taken at. Each mesh Play left as it was (the same
    /// source and the same revision, which every geometry edit renews) takes
    /// `live`'s geometry, moved out, unless its model file was written since
    /// `read_at` (a writer during Play, whose note the edit frame after Stop no
    /// longer drains: `ModelWrites::follow` starts from this restore). Every other
    /// mesh is rebuilt from its reference, each file looked up once.
    pub fn adopting(mut data: crate::serialize::SceneData, live: &mut World, read_at: u64) -> Self {
        let mut imports = Imports::default();
        let written = Stale::new(written_since(read_at).1);
        let unchanged =
            |m: &MeshComponent, l: &MeshComponent| m.same_source(l) && m.revision == l.revision;
        for doc in &mut data.entities {
            if !take_live(live, doc, unchanged, &written, &mut imports) {
                rehydrate_in(doc, &mut imports);
            } else if let Some(mesh) = &mut doc.mesh {
                // Unbound and at rest, as a rebuilt mesh is: the skeleton sync after
                // the restore binds the rebuilt bones.
                mesh.pose_palette.clear();
                mesh.skeleton.bones.clear();
            }
        }
        let read_at = written_since(u64::MAX).0;
        Self { data, read_at }
    }
}

/// Whether `reference`'s model file is among `written` (a primitive's never is).
fn is_written(reference: &Option<String>, written: &Stale) -> bool {
    !written.is_empty()
        && reference
            .as_deref()
            .and_then(super::asset_ref_file)
            .is_some_and(|file| written.matches(&file))
}

/// Give `doc` (recorded without geometry) its live entity's mesh geometry when
/// `keep` holds for the two, and its collider's own meshes by
/// [`restore_collider_meshes`], none of them from a model file among `written`.
/// `false` when `doc`'s Mesh must be rebuilt instead (with its collider, by
/// `rehydrate_entity_mesh`).
pub(crate) fn take_live(
    world: &mut World,
    doc: &mut Entity,
    keep: impl Fn(&MeshComponent, &MeshComponent) -> bool,
    written: &Stale,
    imports: &mut Imports,
) -> bool {
    if let Some(mesh) = &mut doc.mesh {
        let kept = |l: &MeshComponent| keep(mesh, l) && !is_written(&mesh.asset_ref, written);
        let Some(mut live) = world.mesh_mut(doc.id).filter(|l| kept(l)) else {
            return false;
        };
        mesh.adopt_geometry(&mut live);
    }
    restore_collider_meshes(world, doc, written, imports);
    true
}

/// Put back `doc`'s collider's own meshes, recorded without geometry: the kept copy
/// when it has no Mesh (#1657), and the collision mesh (#1960) whatever it has. Both
/// are restored before the box is refit, once, to what it builds from, as
/// `rehydrate_entity_mesh` does: refitting after the first would size it to a kept
/// copy while the collision mesh still had no geometry.
fn restore_collider_meshes(
    world: &mut World,
    doc: &mut Entity,
    written: &Stale,
    imports: &mut Imports,
) {
    let own = doc
        .mesh
        .as_ref()
        .map(|m| m.vertices.iter().map(|v| v.position));
    let own = own.and_then(super::bounds_of);
    let mut rebuilt = false;
    if doc.mesh.is_none() {
        rebuilt |= restore_collider_mesh(world, doc, Which::Kept, written, imports);
    }
    rebuilt |= restore_collider_mesh(world, doc, Which::Collision, written, imports);
    if let Some(collider) = doc.collider.as_mut().filter(|_| rebuilt) {
        super::refit_collider(collider, own);
    }
}

/// Give `doc`'s collider's `which` mesh (recorded without geometry) the live one's
/// when it comes from the same source, from a file not among `written`, and
/// rebuild it from its reference otherwise.
/// Only a Mesh-shaped collider's: another shape never reads it. Whether it was
/// rebuilt, so the caller refits the box.
fn restore_collider_mesh(
    world: &mut World,
    doc: &mut Entity,
    which: Which,
    written: &Stale,
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
            .filter(|l| shared.same_source(l) && !is_written(&shared.asset_ref, written))
        {
            shared.adopt_geometry(live);
            return false;
        }
    }
    super::collider::rebuild(doc.id, collider, which, imports);
    true
}
