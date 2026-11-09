//! src/ecs/bundle.rs — document -> single-placement hecs bundle (#345).
//!
//! `placement.rs`'s `place`/`overwrite` route every whole-entity write through
//! [`build_bundle`] so a document's mandatory + optional columns land in ONE
//! archetype placement, instead of N sequential `insert_one` calls that would
//! each re-migrate the entity through its own archetype (O(k) placement, not
//! O(k²) migration). The other way, [`World::entity_document`] assembles a live
//! entity's columns back into a document (moved here from `core.rs`, #1900).

use std::collections::BTreeMap;

use crate::components::{ColliderComponent, Entity, MaterialAsset, MeshComponent};

use super::world::World;

/// The slim mandatory identity/hierarchy core every GameObject has (#343's
/// "core component"). Everything else — name, transform, scripts, and every
/// optional first-class component — is its own separately-attached hecs
/// column so narrow queries (#346) can skip entities that lack it.
pub(in crate::ecs) struct Core {
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

/// Attach each `Some` value in `$field` to the builder; a `None` simply
/// leaves that column unattached. Keeps `build_bundle` under the function
/// line cap despite the component list being long.
macro_rules! add_present {
    ($b:expr, $($field:expr),* $(,)?) => {
        $( if let Some(v) = $field { $b.add(v); } )*
    };
}

/// Build the full component bundle for one `Entity` document — the mandatory
/// core/name/transform/scripts plus every present optional component. Moves
/// fields directly off `entity` (a series of partial moves) rather than one
/// big destructure pattern, to stay well under the function line cap.
pub(in crate::ecs) fn build_bundle(mut entity: Entity, seq: u64) -> hecs::EntityBuilder {
    let mut b = hecs::EntityBuilder::new();
    b.add(Core::take(&mut entity, seq));
    b.add(entity.name);
    b.add(entity.transform);
    b.add(entity.scripts);
    add_present!(
        b,
        entity.mesh,
        entity.material,
        entity.pending_material,
        entity.animator,
        entity.light,
        entity.collider,
        entity.rigidbody,
        entity.nav_agent,
        entity.nav_obstacle,
        entity.offmesh_link,
        entity.nav_modifier,
        entity.ai_perception,
        entity.camera,
        entity.post_processing,
        entity.particles,
        entity.audio,
        entity.reverb_zone,
        entity.canvas,
        entity.rect_transform,
        entity.image,
        entity.canvas_group,
        entity.rect_mask,
        entity.mask,
        entity.backdrop_filter,
        entity.text,
        entity.selectable,
        entity.layout_group,
        entity.layout_element,
        entity.joint,
        entity.character_controller,
        entity.lod_group,
        entity.light_probe_proxy_volume,
        entity.trail,
        entity.line,
        entity.shape,
        entity.prefab_link,
    );
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
        let core = self.component::<Core>(id)?;
        Some(Entity {
            id: core.id,
            name: (*self.name(id)?).clone(),
            active: core.active,
            is_static: core.is_static,
            layer: core.layer,
            transform: (*self.transform(id)?).clone(),
            mesh: self.mesh(id).map(|c| mesh(&c)),
            material: self.material(id).map(|c| (*c).clone()),
            pending_material: self.component::<MaterialAsset>(id).map(|c| (*c).clone()),
            scripts: (*self.scripts(id)?).clone(),
            animator: self.animator(id).map(|c| (*c).clone()),
            light: self.light(id).map(|c| (*c).clone()),
            collider: self.collider(id).map(|c| collider(&c)),
            rigidbody: self.rigidbody(id).map(|c| (*c).clone()),
            nav_agent: self.nav_agent(id).map(|c| (*c).clone()),
            nav_obstacle: self.nav_obstacle(id).map(|c| (*c).clone()),
            offmesh_link: self.offmesh_link(id).map(|c| (*c).clone()),
            nav_modifier: self.nav_modifier(id).map(|c| (*c).clone()),
            ai_perception: self.ai_perception(id).map(|c| (*c).clone()),
            camera: self.camera(id).map(|c| (*c).clone()),
            post_processing: self.post_processing(id).map(|c| (*c).clone()),
            particles: self.particles(id).map(|c| (*c).clone()),
            audio: self.audio(id).map(|c| (*c).clone()),
            reverb_zone: self.reverb_zone(id).map(|c| (*c).clone()),
            canvas: self.canvas(id).map(|c| (*c).clone()),
            rect_transform: self.rect_transform(id).map(|c| (*c).clone()),
            image: self.image(id).map(|c| (*c).clone()),
            canvas_group: self.canvas_group(id).map(|c| (*c).clone()),
            rect_mask: self.rect_mask(id).map(|c| (*c).clone()),
            mask: self.mask(id).map(|c| (*c).clone()),
            backdrop_filter: self.backdrop_filter(id).map(|c| (*c).clone()),
            text: self.text(id).map(|c| (*c).clone()),
            selectable: self.selectable(id).map(|c| (*c).clone()),
            layout_group: self.layout_group(id).map(|c| (*c).clone()),
            layout_element: self.layout_element(id).map(|c| (*c).clone()),
            joint: self.joint(id).map(|c| (*c).clone()),
            character_controller: self.character_controller(id).map(|c| (*c).clone()),
            lod_group: self.lod_group(id).map(|c| (*c).clone()),
            light_probe_proxy_volume: self.light_probe_proxy_volume(id).map(|c| (*c).clone()),
            trail: self.trail(id).map(|c| (*c).clone()),
            line: self.line(id).map(|c| (*c).clone()),
            shape: self.shape(id).map(|c| (*c).clone()),
            prefab_link: self.prefab_link(id).map(|c| (*c).clone()),
            parent_id: core.parent_id,
            parent_bone: Vec::new(), // document-only, as `bone_refs`: a save fills them
            bone_refs: BTreeMap::new(),
            children: core.children.clone(),
        })
    }
}
