//! src/ecs/names.rs — the name index behind [`World::find_by_name`] (#2089).
//!
//! Unity's `GameObject.Find` is a linear search, and Unity tells users to cache it.
//! Here an agent writes the scripts, and an agent calls it in `Update`, so the
//! World keeps an index instead: name -> the entities carrying it, ordered by
//! insertion sequence (`Core::seq`, the order `World::ids` yields).
//!
//! **Derived from the change log, so no write site can forget it.** The index reads
//! the name column's channel (`String`) through its own cursor: every name write
//! (`World::set_name`) and every whole-entity write (a spawn, a scene or prefab
//! placement, an overwrite, an Undo's restore, a despawn) stamps that channel at
//! the World's funnels, and a clear, a Play restore or an Undo step reads as
//! `Rebuild`. So the index is brought up to date when it is asked, from exactly the
//! ids whose name may have moved, and rebuilt from the whole World when the log
//! cannot say. It opens nothing until the first lookup: a World nobody searches
//! by name pays nothing.
//!
//! **Determinism.** The answer is today's scan's: the first entity, in insertion
//! order, with that name and active in the hierarchy. Activity is asked at lookup
//! (`World::is_active_in_hierarchy`, itself kept), so (de)activations and
//! re-parents need no stamp here. Maps are `BTreeMap`s.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use super::bundle::Core;
use super::changes::{Changes, Cursor};
use super::world::World;

/// The index: its cursor on the name channel, and both directions of the map.
#[derive(Default)]
pub(super) struct NameIndex {
    /// `None` until the first lookup opens it.
    cursor: Option<Cursor<String>>,
    /// Name -> `(seq, id)` of every live entity carrying it, insertion order.
    by_name: BTreeMap<String, BTreeSet<(u64, u32)>>,
    /// Id -> the key it is filed under, so a rename or removal finds its old entry.
    of_id: BTreeMap<u32, (String, u64)>,
}

impl NameIndex {
    fn unfile(&mut self, id: u32) {
        let Some((name, seq)) = self.of_id.remove(&id) else {
            return;
        };
        if let Some(ids) = self.by_name.get_mut(&name) {
            ids.remove(&(seq, id));
            if ids.is_empty() {
                self.by_name.remove(&name);
            }
        }
    }

    fn file(&mut self, id: u32, name: String, seq: u64) {
        let ids = self.by_name.entry(name.clone()).or_default();
        ids.insert((seq, id));
        self.of_id.insert(id, (name, seq));
    }

    /// Re-read `id`'s name and sequence from the World (gone: dropped).
    fn refile(&mut self, world: &World, id: u32) {
        self.unfile(id);
        let seq = world.component::<Core>(id).map(|c| c.seq);
        if let (Some(seq), Some(name)) = (seq, world.component::<String>(id)) {
            self.file(id, String::clone(&name), seq);
        }
    }

    fn rebuild(&mut self, world: &World) {
        self.by_name.clear();
        self.of_id.clear();
        for &id in world.ids() {
            self.refile(world, id);
        }
    }

    /// Bring the index up to what the World holds now.
    fn catch_up(&mut self, world: &World) {
        let changes = match &mut self.cursor {
            Some(cursor) => world.changes(cursor),
            None => {
                self.cursor = Some(world.change_cursor::<String>());
                Changes::Rebuild
            }
        };
        match changes {
            Changes::Rebuild => self.rebuild(world),
            Changes::Ids(ids) => ids.into_iter().for_each(|id| self.refile(world, id)),
        }
    }
}

/// The World's handle on the index: in a `RefCell`, since a lookup takes `&World`.
pub(super) type Names = RefCell<NameIndex>;

impl World {
    /// Find the first entity with this name that is active in the hierarchy
    /// (Unity's `GameObject.Find`), returning its stable id. "First" is insertion
    /// order, as [`World::ids`] yields it. Kept cheap by the name index (module
    /// docs): a lookup costs the ids stamped on the name channel since the last
    /// one, a map lookup, and the same-named entities ahead of the active one.
    pub fn find_by_name(&self, name: &str) -> Option<u32> {
        let mut index = self.names.borrow_mut();
        index.catch_up(self);
        let ids = index.by_name.get(name)?;
        ids.iter()
            .map(|&(_, id)| id)
            .find(|&id| self.is_active_in_hierarchy(id))
    }
}
