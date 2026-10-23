//! src/ecs/despawn.rs — what a despawn clears, so no dead id survives (#1264).
//!
//! An entity id is held in more places than its own row. [`World::despawn`] runs
//! [`World::forget_references`] first, which clears every one the World owns:
//!
//! - the parent's `Core::children` (the id leaves the list);
//! - each child's `Core::parent_id` (the child becomes a root, keeping its world
//!   pose, as Unity's `SetParent(null)` does);
//! - every component field that names an entity, through the component's own
//!   `remap_refs` (`Entity::remap_refs` walks the same set on a document): a
//!   reference to the dead id is dropped.
//!   - `SelectableComponent::target_graphic` and `select_on` (its navigation);
//!   - `JointComponent::connected_body` (the joint then holds to the world, as
//!     Unity's does when its connected body is destroyed);
//!   - `ParticleEmitterComponent`'s sub-emitters (`birth`, `death`, `collision`);
//!   - `LodGroupComponent`'s `renderers`;
//!   - the `target` of a `RectTransformComponent`'s world anchor. A marker with no
//!     target sits at a fixed world point, so the marker is also deactivated: it
//!     stays hidden, as a marker whose target is gone always was.
//!
//! What a dropped reference means is each component's own `None`: a
//! `Selectable` with no `target_graphic` tints itself, a sub-emitter slot or an
//! LOD renderer that is gone draws nothing.
//!
//! `MeshComponent`'s `skeleton.bones` is not cleared: bones go with their skinned
//! entity (`Scene::despawn_skeleton`), and a stale list is re-bound by name.
//! Runtime-only ids (`OffMeshLinkData::owner`, `SubEmitEvent::target`) are never
//! saved and are read through `World::contains`. Scene-level references (the
//! selection, `DontDestroyOnLoad`, lightmap entries, shader overrides) are
//! `Scene::destroy_entity`'s.
//!
//! A new component field holding an entity id joins its component's `remap_refs`
//! and the list above; `scene::skeleton`'s `despawn_refs_tests` builds every kind
//! and checks none survives a despawn.

use crate::components::{
    JointComponent, LodGroupComponent, ParticleEmitterComponent, SelectableComponent,
    TransformComponent,
};

use super::world::World;

impl World {
    /// Clear every reference the World holds to `id`, which is about to be despawned.
    /// See the module docs for the list.
    pub(super) fn forget_references(&mut self, id: u32) {
        if let Some(parent) = self.parent_id(id) {
            self.remove_child(parent, id);
        }
        for child in self.children(id) {
            let world = self.local_to_world(child);
            if let Some(mut t) = self.transform_mut(child) {
                *t = TransformComponent::from_matrix(world);
            }
            self.set_parent_id(child, None);
        }
        let keep = |r: u32| (r != id).then_some(r);
        for (_, s) in self.inner.query_mut::<&mut SelectableComponent>() {
            s.remap_refs(&keep);
        }
        for (_, j) in self.inner.query_mut::<&mut JointComponent>() {
            j.remap_refs(&keep);
        }
        for (_, p) in self.inner.query_mut::<&mut ParticleEmitterComponent>() {
            p.remap_refs(&keep);
        }
        for (_, g) in self.inner.query_mut::<&mut LodGroupComponent>() {
            g.remap_refs(&keep);
        }
        for marker in self.ids_with_rect_transform() {
            let Some(mut r) = self.rect_transform_mut(marker) else {
                continue;
            };
            let followed = r.world_anchor.as_ref().and_then(|a| a.target) == Some(id);
            r.remap_refs(&keep);
            drop(r);
            if followed {
                self.set_active(marker, false);
            }
        }
    }
}
