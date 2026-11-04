//! src/ecs/despawn/index.rs — who names whom: the reverse index a despawn reads
//! instead of scanning every holder (#1708).
//!
//! Per target id, the live entities whose components hold it, by
//! [`EntityRefs::each_held`] (the saved id leaves and a skinned mesh's bone slots).
//! It is derived and never saved. It is opened by the World's first despawn and
//! kept from then on through one change cursor per reference-holding column
//! (`with_ref_components`): each despawn first re-reads the holders written since
//! the last one, so a burst of edits costs one pass over the ids it changed, and a
//! cursor answering `Rebuild` (a load, a clear, Play's restore, an Undo step, a log
//! that dropped its tail) costs one walk over every holder, which is what every
//! despawn paid before.
//!
//! **Exact, not a superset.** After a refresh, a target's holders are exactly the
//! live entities whose components name it: every write to a holding column goes
//! through the funnel, which stamps that column's channel (`ecs::changes`), and a
//! placement, overwrite or removal stamps every column. `index_tests` compares it
//! with the full scan over random edit and despawn sequences.
//!
//! **Determinism.** Ordered maps only; a despawn hands the holders over ascending,
//! the order the scan gave.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{with_ref_components, EntityRefs};
use crate::ecs::{Changes, Cursor, World};

macro_rules! index_walk {
    ($($field:ident: $ty:ty,)*) => {
        /// One cursor per column that can hold an id.
        struct Cursors {
            $($field: Cursor<$ty>,)*
        }

        impl Cursors {
            fn open(world: &mut World) -> Self {
                Self { $($field: world.change_cursor::<$ty>(),)* }
            }

            /// The holders written since the last read, or `None` when any column's
            /// log cannot say. Every cursor is read, so each one moves on.
            fn read(&mut self, world: &mut World) -> Option<BTreeSet<u32>> {
                let mut ids = Some(BTreeSet::new());
                $(match world.changes(&mut self.$field) {
                    Changes::Rebuild => ids = None,
                    Changes::Ids(more) => {
                        if let Some(ids) = &mut ids {
                            ids.extend(more);
                        }
                    }
                })*
                ids
            }
        }

        impl World {
            /// Every id the components of `id` hold, ascending, each once.
            pub(in crate::ecs) fn held_by(&self, id: u32) -> Vec<u32> {
                let mut held = Vec::new();
                $(if let Some(c) = self.component::<$ty>(id) {
                    c.each_held(&mut |r| held.push(r));
                })*
                held.sort_unstable();
                held.dedup();
                held
            }
        }
    };
}
with_ref_components!(index_walk);

/// The reverse index. See the module docs.
pub(in crate::ecs) struct RefIndex {
    cursors: Cursors,
    /// Target id -> the live entities holding it.
    by_target: BTreeMap<u32, BTreeSet<u32>>,
    /// Holder id -> what it held when last read, ascending.
    by_holder: BTreeMap<u32, Vec<u32>>,
    /// Holders re-read or handed to a despawn since opening: what a test counts
    /// a despawn's work by.
    pub(in crate::ecs) touched: u64,
}

impl RefIndex {
    /// Open the cursors and index every holder.
    pub(in crate::ecs) fn open(world: &mut World) -> Self {
        let mut index = Self {
            cursors: Cursors::open(world),
            by_target: BTreeMap::new(),
            by_holder: BTreeMap::new(),
            touched: 0,
        };
        index.rebuild(world);
        index
    }

    /// Catch up with every holder written since the last refresh.
    pub(in crate::ecs) fn refresh(&mut self, world: &mut World) {
        match self.cursors.read(world) {
            None => self.rebuild(world),
            Some(ids) => ids.into_iter().for_each(|id| self.reindex(world, id)),
        }
    }

    fn rebuild(&mut self, world: &World) {
        self.by_target.clear();
        self.by_holder.clear();
        for holder in world.ref_holders() {
            self.reindex(world, holder);
        }
    }

    /// Re-read what `holder` holds now (nothing, once it is gone).
    fn reindex(&mut self, world: &World, holder: u32) {
        self.touched += 1;
        let held = if world.contains(holder) {
            world.held_by(holder)
        } else {
            Vec::new()
        };
        let old = self.by_holder.get(&holder).map_or(&[][..], Vec::as_slice);
        if old == held.as_slice() {
            return;
        }
        for t in old {
            if let Some(holders) = self.by_target.get_mut(t) {
                holders.remove(&holder);
                if holders.is_empty() {
                    self.by_target.remove(t);
                }
            }
        }
        for &t in &held {
            self.by_target.entry(t).or_default().insert(holder);
        }
        match held.is_empty() {
            true => self.by_holder.remove(&holder),
            false => self.by_holder.insert(holder, held),
        };
    }

    /// The live entities outside `dead` holding any of its ids, ascending. Their
    /// targets in `dead` are dropped from the index: the despawn clears them, and
    /// each holder it writes is re-read at the next refresh.
    pub(in crate::ecs) fn take_naming(&mut self, dead: &BTreeSet<u32>) -> Vec<u32> {
        let mut naming = BTreeSet::new();
        for d in dead {
            if let Some(holders) = self.by_target.remove(d) {
                naming.extend(holders.into_iter().filter(|h| !dead.contains(h)));
            }
        }
        self.touched += naming.len() as u64;
        naming.into_iter().collect()
    }

    /// The same answer without dropping anything: what the tests compare.
    #[cfg(test)]
    pub(in crate::ecs) fn naming(&self, dead: &BTreeSet<u32>) -> Vec<u32> {
        let holders = dead.iter().filter_map(|d| self.by_target.get(d)).flatten();
        let naming: BTreeSet<u32> = holders.filter(|h| !dead.contains(h)).copied().collect();
        naming.into_iter().collect()
    }
}
