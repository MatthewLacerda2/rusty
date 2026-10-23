//! src/ecs/despawn.rs — what a despawn clears, so no dead id survives (#1264).
//!
//! An entity id is held in more places than its own row. [`World::despawn`] runs
//! [`World::forget_references`] first, which clears every one the World owns:
//!
//! - the parent's `Core::children` (the id leaves the list);
//! - each child's `Core::parent_id` (the child becomes a root, keeping its world
//!   pose, as Unity's `SetParent(null)` does);
//! - every component id leaf that names it, found by the one walk over id fields
//!   (`components::entity::refs`, which lists them): the leaf takes its field's
//!   `None`. A Joint then holds to the world (Unity's, when its connected body is
//!   destroyed), a Selectable with no `target_graphic` tints itself, a lost
//!   sub-emitter or LOD renderer draws nothing, a marker whose target is gone is
//!   `target_lost` and hidden (its scripts keep running, so it can be pinned
//!   again), and a prefab override naming it becomes `null`.
//!
//! The World walk below goes over the same component list the document walk does
//! ([`with_ref_components`]), so the two cannot disagree. Scene-level references
//! (the selection, `DontDestroyOnLoad`, lightmap entries, shader overrides) are
//! `Scene::despawn_one`'s.

use std::collections::BTreeSet;

use crate::components::{with_ref_components, EntityRefs, RefVisit, TransformComponent};

use super::world::World;

macro_rules! world_walk {
    ($($field:ident: $ty:ty,)*) => {
        impl World {
            /// Visit every component id slot of entity `id`, with its pointer under
            /// the entity, as `Entity::visit_refs` does on its document.
            pub fn visit_refs(&mut self, id: u32, visit: &mut RefVisit<'_>) {
                $(if let Some(mut c) = self.component_mut::<$ty>(id) {
                    let prefix = concat!("/", stringify!($field));
                    c.visit_refs(&mut |p, r| visit(&format!("{prefix}{p}"), r));
                })*
            }

            /// Whether any component of entity `id` names `target`.
            pub fn names(&self, id: u32, target: u32) -> bool {
                false $(|| self.component::<$ty>(id).is_some_and(|c| c.names(target)))*
            }

            /// Every entity carrying a component that can hold an id, in id order.
            pub fn ref_holders(&self) -> Vec<u32> {
                let mut ids = BTreeSet::new();
                $(ids.extend(self.ids_with_component::<$ty>());)*
                ids.into_iter().collect()
            }
        }
    };
}
with_ref_components!(world_walk);

impl World {
    /// Clear every reference the World holds to `id`, which is about to be despawned.
    /// See the module docs. Entities that name it are found with a read pass first,
    /// so a despawn nothing points at takes no mutable borrow.
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
        let naming: Vec<u32> = self
            .ref_holders()
            .into_iter()
            .filter(|&e| self.names(e, id))
            .collect();
        for e in naming {
            self.visit_refs(e, &mut |_, r| {
                if *r == Some(id) {
                    *r = None;
                }
            });
        }
    }
}
