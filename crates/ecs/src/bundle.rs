//! A document to one hecs bundle, so a whole entity lands in one archetype placement (not one
//! migration per `insert_one`), and a live entity's columns back into a document.

use std::collections::BTreeMap;

use rusty_components::registry::first_class;
use rusty_components::{ColliderComponent, Entity, MaterialAsset, MeshComponent};

use super::world::World;

pub(crate) struct Core {
    pub id: u32,
    /// Insertion sequence, kept across `overwrite`: narrow queries sort on it, as hecs iterates by
    /// archetype.
    pub seq: u64,
    pub active: bool,
    pub is_static: bool,
    pub layer: u8,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

impl Core {
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

macro_rules! bundle_rows {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        fn add_components(b: &mut hecs::EntityBuilder, entity: Entity) {
            $(if let Some(v) = entity.$field { b.add(v); })*
        }

        impl Documented<'_> {
            // One struct literal: a field added to `Entity` and not copied here fails to compile.
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

struct Documented<'w> {
    world: &'w World,
    id: u32,
    mesh: fn(&MeshComponent) -> MeshComponent,
    collider: fn(&ColliderComponent) -> ColliderComponent,
}

first_class!(bundle_rows);

/// Keeps the skin: a save reads its joints to strip the bones a load rebuilds.
fn saved_mesh(mesh: &MeshComponent) -> MeshComponent {
    MeshComponent {
        skin: mesh.skin.clone(),
        ..mesh.without_geometry()
    }
}

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
    pub fn entity_document(&self, id: u32) -> Option<Entity> {
        self.document(id, MeshComponent::clone, ColliderComponent::clone)
    }

    pub fn saved_document(&self, id: u32) -> Option<Entity> {
        self.document(id, saved_mesh, ColliderComponent::without_geometry)
    }

    /// What a save reads (`edit::serialize`): no vertices, which a load rebuilds.
    pub fn collect_documents(&self) -> Vec<Entity> {
        let ids = self.ids().iter();
        ids.filter_map(|&id| self.saved_document(id)).collect()
    }

    pub fn reference_document(&self, id: u32) -> Option<Entity> {
        self.document(
            id,
            MeshComponent::reference_only,
            ColliderComponent::without_geometry,
        )
    }

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
