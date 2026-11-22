//! What a despawn clears so no dead id survives: the parent's `children`, each child's parent
//! link (a root at its world pose, as Unity's `SetParent(null)`), and every component id leaf
//! (its `None`), found by the reverse index. Scene-level references are `Scene::despawn_batch`'s.

#[cfg(test)]
mod cost_tests;
mod index;
#[cfg(test)]
mod index_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use rusty_components::{with_ref_components, EntityRefs, RefVisit, TransformComponent};

use super::world::World;
pub(crate) use index::RefIndex;

macro_rules! world_walk {
    ($($field:ident: $ty:ty,)*) => {
        impl World {
            pub fn visit_refs(&mut self, id: u32, visit: &mut RefVisit<'_>) {
                $(if let Some(mut c) = self.component_mut::<$ty>(id) {
                    let prefix = concat!("/", stringify!($field));
                    c.visit_refs(&mut |p, r| visit(&format!("{prefix}{p}"), r));
                })*
            }

            fn forget_in(&mut self, id: u32, dead: &dyn Fn(u32) -> bool) {
                $(if let Some(mut c) = self.component_mut::<$ty>(id) {
                    c.forget(dead);
                })*
            }

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
    /// A test hook: the engine despawns in batches ([`World::despawn_all`]).
    #[doc(hidden)]
    pub fn despawn(&mut self, id: u32) {
        self.despawn_all(&BTreeSet::from([id]));
    }

    pub fn despawn_all(&mut self, ids: &BTreeSet<u32>) {
        self.forget_references(ids);
        self.remove_rows(ids);
    }

    /// One pass for the batch: a child dying with its parent is not re-rooted.
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

    pub(crate) fn with_ref_index<R>(&mut self, f: impl FnOnce(&mut RefIndex) -> R) -> R {
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

    /// The full scan the index replaced: what `index_tests` holds it to.
    #[cfg(test)]
    pub(crate) fn naming_by_scan(&self, dead: &BTreeSet<u32>) -> Vec<u32> {
        let named = |e: &u32| self.held_by(*e).iter().any(|r| dead.contains(r));
        let holders = self.ref_holders().into_iter();
        holders.filter(|e| !dead.contains(e) && named(e)).collect()
    }
}
