//! What one of several loaded scenes owes its file, so an untouched one saves byte-identical:
//! the file's ids and names (a join renames some), and what the World took none of.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use crate::serialize::SceneData;
use crate::Scene;

#[derive(Clone, Default)]
pub struct SceneFile {
    /// World id to the file's.
    ids: RefCell<BTreeMap<u32, u32>>,
    next_written: Cell<u32>,
    /// Past every id it took, rebuilt bones included.
    next_free: Cell<u32>,
    pub materials: BTreeMap<String, String>,
    pub meshes: BTreeMap<String, String>,
    /// `None` for the first scene, whose layers and bakes the World holds.
    pub carried: Option<Box<SceneData>>,
}

impl std::fmt::Debug for SceneFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneFile")
            .field("ids", &self.ids)
            .field("next_written", &self.next_written)
            .field("materials", &self.materials)
            .field("meshes", &self.meshes)
            .field("carried", &self.carried.is_some())
            .finish()
    }
}

impl SceneFile {
    pub fn new(ids: BTreeMap<u32, u32>, next_written: u32, next_free: u32) -> Self {
        let next_free = next_free.max(next_written);
        let next_free = ids.values().map(|&v| v + 1).fold(next_free, u32::max);
        Self {
            ids: RefCell::new(ids),
            next_written: Cell::new(next_written),
            next_free: Cell::new(next_free),
            ..Default::default()
        }
    }

    /// Each member's file id, a new one past the file's own; [`Self::keep`] keeps them.
    pub fn file_ids(&self, members: &[u32]) -> (BTreeMap<u32, u32>, u32) {
        let held = self.ids.borrow();
        let mut next = self.next_free.get();
        let mut given = false;
        let mut out = BTreeMap::new();
        for &id in members {
            let file = match held.get(&id) {
                Some(&file) => file,
                None => {
                    given = true;
                    next += 1;
                    next - 1
                }
            };
            out.insert(id, file);
        }
        let written = match given {
            true => next,
            false => self.next_written.get(),
        };
        (out, written)
    }

    pub fn holds(&self, id: u32) -> bool {
        self.ids.borrow().contains_key(&id)
    }

    pub fn keep(&self, ids: &BTreeMap<u32, u32>, next_written: u32) {
        let mut held = self.ids.borrow_mut();
        for (&world, &file) in ids {
            held.insert(world, file);
            if file >= self.next_free.get() {
                self.next_free.set(file + 1);
            }
        }
        self.next_written.set(next_written);
    }
}

impl Scene {
    /// The World as its one scene's file, before its first additive join.
    pub fn file_of_world(&self) -> SceneFile {
        let ids = self.world.ids().iter().map(|&id| (id, id)).collect();
        let next = crate::skeleton::saved_next_id(self);
        let mut file = SceneFile::new(ids, next, self.world.next_id());
        file.materials = self
            .materials
            .keys()
            .map(|k| (k.clone(), k.clone()))
            .collect();
        file.meshes = self.meshes.keys().map(|k| (k.clone(), k.clone())).collect();
        file
    }
}
