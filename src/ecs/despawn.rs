//! src/ecs/despawn.rs — what a despawn clears, so no dead id survives (#1264).
//!
//! An entity id is held in more places than its own row. [`World::despawn`] (one
//! entity) and [`World::despawn_all`] (a batch, in one pass) run
//! [`World::forget_references`] first, which clears every one the World owns:
//!
//! - the parent's `Core::children` (the id leaves the list);
//! - each child's `Core::parent_id` (the child becomes a root, keeping its world
//!   pose, as Unity's `SetParent(null)` does);
//! - every component id leaf that names it, found by the one walk over id fields
//!   (`components::entity::refs`, which lists them): the leaf takes its field's
//!   `None` ([`EntityRefs::forget`]). A Joint then holds to the world (Unity's,
//!   when its connected body is destroyed), a Selectable with no `target_graphic` tints itself, a lost
//!   sub-emitter or LOD renderer draws nothing, a marker whose target is gone is
//!   `target_lost` and hidden (its scripts keep running, so it can be given a target
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

            /// Whether any component of entity `id` names an entity `dead` holds for.
            fn names_any(&self, id: u32, dead: &dyn Fn(u32) -> bool) -> bool {
                false $(|| self.component::<$ty>(id).is_some_and(|c| c.names_any(dead)))*
            }

            /// [`EntityRefs::forget`] on every component of entity `id`.
            fn forget_in(&mut self, id: u32, dead: &dyn Fn(u32) -> bool) {
                $(if let Some(mut c) = self.component_mut::<$ty>(id) {
                    c.forget(dead);
                })*
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
    /// Despawn an entity by stable id: [`World::despawn_all`] of one.
    pub fn despawn(&mut self, id: u32) {
        self.despawn_all(&BTreeSet::from([id]));
    }

    /// Despawn every entity of `ids` together, every reference the World holds to
    /// them cleared first, in one pass for the batch. A dead id reads as identity.
    pub fn despawn_all(&mut self, ids: &BTreeSet<u32>) {
        self.forget_references(ids);
        self.remove_rows(ids);
    }

    /// Clear every reference the World holds to the entities of `dead`, which are
    /// about to be despawned together. See the module docs. One pass serves the
    /// whole batch: the holders are listed once and read first, so a despawn nothing
    /// points at takes no mutable borrow, and a child dying with its parent is not
    /// re-rooted.
    fn forget_references(&mut self, dead: &BTreeSet<u32>) {
        for &id in dead {
            if let Some(parent) = self.parent_id(id).filter(|p| !dead.contains(p)) {
                self.remove_child(parent, id);
            }
            for child in self.children(id) {
                if dead.contains(&child) {
                    continue;
                }
                let world = self.local_to_world(child);
                if let Some(mut t) = self.transform_mut(child) {
                    *t = TransformComponent::from_matrix(world);
                }
                self.set_parent_id(child, None);
            }
        }
        let is_dead = |r: u32| dead.contains(&r);
        let naming: Vec<u32> = self
            .ref_holders()
            .into_iter()
            .filter(|e| !dead.contains(e) && self.names_any(*e, &is_dead))
            .collect();
        for e in naming {
            self.forget_in(e, &is_dead);
        }
    }
}
