//! crates/ecs/src/bundle.rs — document -> single-placement hecs bundle (#345).
//!
//! `placement.rs`'s `place`/`overwrite` route every whole-entity write through
//! [`build_bundle`] so a document's mandatory + optional columns land in ONE
//! archetype placement, instead of N sequential `insert_one` calls that would
//! each re-migrate the entity through its own archetype (O(k) placement, not
//! O(k²) migration). The other way, [`World::entity_document`] assembles a live
//! entity's columns back into a document (moved here from `core.rs`, #1900).

use std::collections::BTreeMap;

use rusty_components::registry::first_class;
use rusty_components::{ColliderComponent, Entity, MaterialAsset, MeshComponent};

use super::world::World;

/// The slim mandatory identity/hierarchy core every GameObject has (#343's
/// "core component"). Everything else — name, transform, scripts, and every
/// optional first-class component — is its own separately-attached hecs
/// column so narrow queries (#346) can skip entities that lack it.
pub(crate) struct Core {
    pub id: u32,
    /// Monotonic insertion sequence, assigned by `World::place` and preserved
    /// across `overwrite`. Narrow queries (#346) sort their matches by it to
    /// recover insertion order — hecs iterates by archetype, and the
    /// determinism contract (physics pair ordering, replay byte-identity)
    /// requires insertion order.
    pub seq: u64,
    pub active: bool,
    pub is_static: bool,
    pub layer: u8,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

impl Core {
    /// The core of `entity` at insertion sequence `seq`, moving its children out.
    fn take(entity: &mut Entity, seq: u64) -> Self {
        Self {
            id: entity.id,
            seq,
            active: entity.active,
            is_static: entity.is_static,
            layer: entity.layer,
            parent_id: entity.parent_id,
            children: std::mem::take(&mut entity.children),
        }
    }
}

/// Generates [`add_components`] and [`Documented::entity`] from the registry
/// rows: each row's column into a bundle, and back out of a live entity.
macro_rules! bundle_rows {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        /// Attach each first-class component `entity` carries to the builder; a
        /// `None` leaves that column unattached.
        fn add_components(b: &mut hecs::EntityBuilder, entity: Entity) {
            $(if let Some(v) = entity.$field { b.add(v); })*
        }

        impl Documented<'_> {
            /// The entity's document, as ONE struct literal: a field added to
            /// `Entity` and not copied here is a compile error, not a value every
            /// save, Play/Stop snapshot and prefab hold silently resets.
            fn entity(&self) -> Option<Entity> {
                let (world, id) = (self.world, self.id);
                let core = world.component::<Core>(id)?;
                Some(Entity {
                    id: core.id,
                    name: (*world.name(id)?).clone(),
                    active: core.active,
                    is_static: core.is_static,
                    layer: core.layer,
                    transform: (*world.transform(id)?).clone(),
                    pending_material: world.component::<MaterialAsset>(id).map(|c| (*c).clone()),
                    scripts: (*world.scripts(id)?).clone(),
                    $($field: document_field!(self, $field),)*
                    prefab_link: world.prefab_link(id).map(|c| (*c).clone()),
                    parent_id: core.parent_id,
                    // Document-only: a save fills them.
                    parent_bone: Vec::new(),
                    bone_refs: BTreeMap::new(),
                    children: core.children.clone(),
                })
            }
        }
    };
}

/// One row's column as a document field: cloned, but for the Mesh and the
/// Collider, copied by the functions the caller chose.
macro_rules! document_field {
    ($d:expr, mesh) => {
        $d.world.mesh($d.id).map(|c| ($d.mesh)(&c))
    };
    ($d:expr, collider) => {
        $d.world.collider($d.id).map(|c| ($d.collider)(&c))
    };
    ($d:expr, $field:ident) => {
        $d.world.$field($d.id).map(|c| (*c).clone())
    };
}

/// One live entity being copied into a document ([`World::document`]).
struct Documented<'w> {
    world: &'w World,
    id: u32,
    mesh: fn(&MeshComponent) -> MeshComponent,
    collider: fn(&ColliderComponent) -> ColliderComponent,
}

first_class!(bundle_rows);

/// Build the full component bundle for one `Entity` document — the mandatory
/// core/name/transform/scripts plus every present optional component, so the
/// whole entity lands in ONE archetype placement.
pub(crate) fn build_bundle(mut entity: Entity, seq: u64) -> hecs::EntityBuilder {
    let mut b = hecs::EntityBuilder::new();
    b.add(Core::take(&mut entity, seq));
    b.add(std::mem::take(&mut entity.name));
    b.add(std::mem::take(&mut entity.transform));
    b.add(std::mem::take(&mut entity.scripts));
    if let Some(v) = entity.pending_material.take() {
        b.add(v);
    }
    if let Some(v) = entity.prefab_link.take() {
        b.add(v);
    }
    add_components(&mut b, entity);
    b
}

impl World {
    /// Assemble one live entity's columns into a document (serialization).
    pub fn entity_document(&self, id: u32) -> Option<Entity> {
        self.document(id, MeshComponent::clone, ColliderComponent::clone)
    }

    /// [`World::entity_document`] with the mesh's reference and none of its
    /// geometry ([`MeshComponent::reference_only`]), and a collider's shared mesh
    /// without its geometry either: what a prefab diff reads.
    pub fn reference_document(&self, id: u32) -> Option<Entity> {
        self.document(
            id,
            MeshComponent::reference_only,
            ColliderComponent::without_geometry,
        )
    }

    /// The document of entity `id`, its mesh copied by `mesh` and its collider by
    /// `collider` (whose shared mesh may carry geometry, #1657).
    pub(super) fn document(
        &self,
        id: u32,
        mesh: fn(&MeshComponent) -> MeshComponent,
        collider: fn(&ColliderComponent) -> ColliderComponent,
    ) -> Option<Entity> {
        Documented {
            world: self,
            id,
            mesh,
            collider,
        }
        .entity()
    }
}
