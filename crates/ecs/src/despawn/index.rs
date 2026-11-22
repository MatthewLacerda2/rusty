use std::collections::{BTreeMap, BTreeSet};

use crate::{Changes, Cursor, World};
use rusty_components::{with_ref_components, EntityRefs};

macro_rules! index_walk {
    ($($field:ident: $ty:ty,)*) => {
        struct Cursors {
            $($field: Cursor<$ty>,)*
        }

        impl Cursors {
            fn open(world: &mut World) -> Self {
                Self { $($field: world.change_cursor::<$ty>(),)* }
            }

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
            pub(crate) fn held_by(&self, id: u32) -> Vec<u32> {
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

pub(crate) struct RefIndex {
    cursors: Cursors,
    by_target: BTreeMap<u32, BTreeSet<u32>>,
    by_holder: BTreeMap<u32, Vec<u32>>,
    #[cfg(test)]
    pub(crate) touched: u64,
}

impl RefIndex {
    pub(crate) fn open(world: &mut World) -> Self {
        let mut index = Self {
            cursors: Cursors::open(world),
            by_target: BTreeMap::new(),
            by_holder: BTreeMap::new(),
            #[cfg(test)]
            touched: 0,
        };
        index.rebuild(world);
        index
    }

    pub(crate) fn refresh(&mut self, world: &mut World) {
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

    fn reindex(&mut self, world: &World, holder: u32) {
        #[cfg(test)]
        {
            self.touched += 1;
        }
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

    pub(crate) fn naming(&self, dead: &BTreeSet<u32>) -> Vec<u32> {
        let holders = dead.iter().filter_map(|d| self.by_target.get(d)).flatten();
        let naming: BTreeSet<u32> = holders.filter(|h| !dead.contains(h)).copied().collect();
        naming.into_iter().collect()
    }
}
