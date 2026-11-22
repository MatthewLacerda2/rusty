use super::{rehydrate_in, Imports, RehydratedScene, Which};
use rusty_components::{ColliderComponent, ColliderShape, Entity, MeshComponent, SharedMesh};
use rusty_ecs::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// Undo's: it runs on an edit frame, whose model follow catches a write.
    SameSource,
    /// Stop's and the prefab propagation's: during Play a model can change on disk unwatched.
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
    pub fn adopting(mut data: crate::serialize::SceneData, live: &mut World) -> Self {
        // Before the lookups, so a model written while they ran is still followed.
        let read_at = rusty_core::fs::written::written_since(u64::MAX).0;
        let mut imports = Imports::default();
        for doc in &mut data.entities {
            adopt_or_rehydrate(live, doc, Keep::Current, &mut imports);
        }
        Self {
            data,
            read_at,
            imports,
        }
    }
}

fn unbind(doc: &mut Entity) {
    if let Some(mesh) = &mut doc.mesh {
        mesh.pose_palette.clear();
        mesh.skeleton.bones.clear();
    }
}

pub fn adopt_or_rehydrate(
    world: &mut World,
    doc: &mut Entity,
    keep: Keep,
    imports: &mut Imports,
) -> bool {
    if let Some(mesh) = &mut doc.mesh {
        super::hold_fields(doc.id, mesh);
    }
    if !take_live(world, doc, keep, imports) {
        rehydrate_in(doc, imports);
        return true;
    }
    // Undo's adopted mesh keeps its live bones and its recorded box.
    if keep == Keep::Current {
        settle(doc);
    }
    false
}

fn settle(doc: &mut Entity) {
    unbind(doc);
    let mesh_shaped = |c: &&mut ColliderComponent| matches!(c.shape, ColliderShape::Mesh { .. });
    let Some(collider) = doc.collider.as_mut().filter(mesh_shaped) else {
        return;
    };
    let own = doc
        .mesh
        .as_ref()
        .map(|m| m.vertices.iter().map(|v| v.position));
    super::refit_collider(collider, own.and_then(super::bounds_of));
}

fn take_live(world: &mut World, doc: &mut Entity, keep: Keep, imports: &mut Imports) -> bool {
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
    // Once both are back: a refit between would size the box to the kept copy alone.
    if let Some(collider) = doc.collider.as_mut().filter(|_| rebuilt) {
        super::refit_collider(collider, own);
    }
}

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
