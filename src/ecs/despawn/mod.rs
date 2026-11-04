//! src/ecs/despawn/ — what a despawn clears, so no dead id survives (#1264), found
//! without scanning the level (#1708).
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
//!   again), and a prefab override naming it becomes `null`;
//! - a skinned mesh's bone slot naming it, which goes missing (`None`, #1273): the
//!   slots are cleared in this pass, never saved, so they are not in the walk.
//!
//! The World walk below goes over the same component list the document walk does
//! ([`with_ref_components`]), so the two cannot disagree. Scene-level references
//! (the selection, `DontDestroyOnLoad`, lightmap entries, shader overrides) are
//! `Scene::despawn_batch`'s.
//!
//! **Cost.** A despawn pays for what it kills, not for the level: the components
//! naming a dead id come from the reverse index (`index.rs`), not a walk over every
//! holder; only the dead ids and their children's subtrees lose their kept
//! `activeInHierarchy` (`ecs::activity`); and each leaves `World::order` by its
//! slot, found by its sequence stamp (`World::remove_rows`).

#[cfg(test)]
mod cost_tests;
mod index;
#[cfg(test)]
mod index_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use crate::components::{with_ref_components, EntityRefs, RefVisit, TransformComponent};

use super::world::World;
pub(in crate::ecs) use index::RefIndex;

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
    #[cfg(test)]
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
    /// whole batch: the holders naming any of them come from the index at once, so
    /// a despawn nothing points at takes no mutable borrow of a holder, and a child
    /// dying with its parent is not re-rooted.
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
        let naming = self.with_ref_index(|index| {
            let naming = index.naming(dead);
            #[cfg(test)]
            {
                index.touched += naming.len() as u64;
            }
            naming
        });
        for e in naming {
            self.forget_in(e, &is_dead);
        }
    }

    /// Run `f` on the reverse index, caught up with every write since the last
    /// despawn (opened, and every holder indexed, the first time).
    pub(in crate::ecs) fn with_ref_index<R>(&mut self, f: impl FnOnce(&mut RefIndex) -> R) -> R {
        let mut index = match self.refs.take() {
            Some(mut index) => {
                index.refresh(self);
                index
            }
            None => RefIndex::open(self),
        };
        let out = f(&mut index);
        self.refs = Some(index);
        out
    }

    /// Which live entities outside `dead` name one of its ids, by the full scan
    /// every despawn ran before the index (#1708): what `index_tests` holds the
    /// index to.
    #[cfg(test)]
    pub(in crate::ecs) fn naming_by_scan(&self, dead: &BTreeSet<u32>) -> Vec<u32> {
        let named = |e: &u32| self.held_by(*e).iter().any(|r| dead.contains(r));
        let holders = self.ref_holders().into_iter();
        holders.filter(|e| !dead.contains(e) && named(e)).collect()
    }
}
