//! The name index behind [`World::find_by_name`], derived from the name channel's cursor so no
//! write site can forget it; opened by the first lookup. Activity is asked at lookup, so
//! (de)activations and re-parents need no stamp here.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use super::bundle::Core;
use super::changes::{Changes, Cursor};
use super::world::World;

#[derive(Default)]
pub(super) struct NameIndex {
    cursor: Option<Cursor<String>>,
    /// Ordered by `Core::seq`, the order `World::ids` yields.
    by_name: BTreeMap<String, BTreeSet<(u64, u32)>>,
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

pub(super) type Names = RefCell<NameIndex>;

impl World {
    pub fn find_by_name(&self, name: &str) -> Option<u32> {
        let mut index = self.names.borrow_mut();
        index.catch_up(self);
        let ids = index.by_name.get(name)?;
        ids.iter()
            .map(|&(_, id)| id)
            .find(|&id| self.is_active_in_hierarchy(id))
    }
}
