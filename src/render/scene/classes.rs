//! src/render/scene/classes.rs — the material table's value classes (#1706): the
//! slots whose records hold equal bytes, so draws naming different slots of equal
//! value still batch, as they did when a key held the record's words.
//!
//! A slot is per (scene, library material, variant), and the ordinary script paths
//! (`Material.Set*`, `Scene.AddComponent(id, "Material")`, a glTF naming no
//! material) give each entity its own `entity_<id>_material`. So 600 boxes each
//! tinted one grey are 600 slots of one value: one class, one batch. Each frame's
//! batching ([`FrameDraws::build`], the shadow sweeps) turns a slot into its class's
//! representative before it sorts, and the per-draw uniform names that slot, which
//! holds the same bytes.
//!
//! A write moves one slot between classes in O(log n), no member walked: a slot
//! names its class's index, and a class keeps its members and its representative
//! (the least member). A record-only edit therefore still derives no object.
//!
//! [`FrameDraws::build`]: crate::render::draw::batch::FrameDraws::build

use std::collections::{BTreeMap, BTreeSet};

use crate::render::MaterialData;

/// A record's bytes, as words: what two slots must share to be one class.
type Value = [u32; WORDS];
const WORDS: usize = std::mem::size_of::<MaterialData>() / 4;

/// No class: a slot never written, or freed.
const NONE: u32 = u32::MAX;

struct Class {
    value: Value,
    members: BTreeSet<u32>,
    /// The least member: what a draw naming any member names.
    rep: u32,
}

/// Every written slot's class.
#[derive(Default)]
pub(crate) struct Classes {
    /// By slot: the index of its class in `classes`, or [`NONE`].
    of: Vec<u32>,
    classes: Vec<Class>,
    by_value: BTreeMap<Value, u32>,
    /// Indices of `classes` emptied, reused first.
    free: Vec<u32>,
}

impl Classes {
    /// `slot` now holds `data`.
    pub(crate) fn set(&mut self, slot: u32, data: &MaterialData) {
        let value: Value = bytemuck::cast(*data);
        let at = slot as usize;
        if let Some(&c) = self.of.get(at) {
            if c != NONE && self.classes[c as usize].value == value {
                return;
            }
        }
        self.clear(slot);
        let c = match self.by_value.get(&value) {
            Some(&c) => c,
            None => self.open(value),
        };
        let class = &mut self.classes[c as usize];
        class.members.insert(slot);
        class.rep = class.rep.min(slot);
        if self.of.len() <= at {
            self.of.resize(at + 1, NONE);
        }
        self.of[at] = c;
    }

    /// `slot` holds nothing now (it was freed).
    pub(crate) fn clear(&mut self, slot: u32) {
        let Some(c) = self.of.get_mut(slot as usize) else {
            return;
        };
        let c = std::mem::replace(c, NONE);
        if c == NONE {
            return;
        }
        let class = &mut self.classes[c as usize];
        class.members.remove(&slot);
        match class.members.first() {
            Some(&least) => class.rep = least,
            None => {
                self.by_value.remove(&class.value);
                self.free.push(c);
            }
        }
    }

    /// The slot a draw naming `slot` names: the least slot holding equal bytes
    /// (`slot` itself when it was never written). Two array reads.
    pub(crate) fn rep(&self, slot: u32) -> u32 {
        match self.of.get(slot as usize) {
            Some(&c) if c != NONE => self.classes[c as usize].rep,
            _ => slot,
        }
    }

    /// A new, empty class holding `value`.
    fn open(&mut self, value: Value) -> u32 {
        let class = Class {
            value,
            members: BTreeSet::new(),
            rep: u32::MAX,
        };
        let c = match self.free.pop() {
            Some(c) => {
                self.classes[c as usize] = class;
                c
            }
            None => {
                self.classes.push(class);
                self.classes.len() as u32 - 1
            }
        };
        self.by_value.insert(value, c);
        c
    }

    /// How many classes hold a slot.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.by_value.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(v: f32) -> MaterialData {
        MaterialData::flat([v, v, v, 1.0])
    }

    #[test]
    fn equal_bytes_share_the_least_slot_and_an_edit_moves_one() {
        let mut c = Classes::default();
        for slot in 1..=4 {
            c.set(slot, &grey(0.5));
        }
        assert_eq!((1..=4).map(|s| c.rep(s)).collect::<Vec<_>>(), [1; 4]);
        assert_eq!(c.len(), 1);
        // The representative is edited: the rest keep drawing as one.
        c.set(1, &grey(0.9));
        assert_eq!([c.rep(1), c.rep(2), c.rep(3), c.rep(4)], [1, 2, 2, 2]);
        assert_eq!(c.len(), 2);
        // Set back, it joins again and is the least once more.
        c.set(1, &grey(0.5));
        assert_eq!([c.rep(1), c.rep(4)], [1, 1]);
        assert_eq!(c.len(), 1, "the emptied class is gone");
        // The representative is freed while the others draw.
        c.clear(1);
        assert_eq!([c.rep(2), c.rep(4)], [2, 2]);
        assert_eq!(c.rep(9), 9, "a slot never written names itself");
    }

    #[test]
    fn a_class_emptied_is_reused_and_rewriting_a_value_changes_nothing() {
        let mut c = Classes::default();
        c.set(3, &grey(0.1));
        c.set(3, &grey(0.1));
        c.set(3, &grey(0.2));
        assert_eq!((c.len(), c.classes.len()), (1, 1), "one class, reused");
        c.clear(3);
        c.clear(3);
        assert_eq!(c.len(), 0);
    }
}
