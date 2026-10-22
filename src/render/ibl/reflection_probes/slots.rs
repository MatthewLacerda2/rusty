//! Which reflection probe sits in which layer of the probe cube array (#1206): a pure
//! slot table, GPU-free so its reuse and growth rules are unit-tested headlessly.
//!
//! A slot keeps its cube until another probe needs the layer, so a camera walking
//! through a level copies only the probes that come into range, never the whole set.

/// The most reflection probes shaded in one frame: the array's layer budget and the
/// length of `LightingUniforms.refl_probes`. The frame keeps the probes nearest the
/// camera; an object whose probe is past them reflects the sky. 32 cubes are 192
/// array layers (under wgpu's default limit of 256) and, at the bake's 128-texel
/// face, about 17 MB of RGBA8 with mips.
pub(crate) const MAX_REFLECTION_PROBES: usize = 32;

/// One frame's plan for the array: the slot each wanted cube lands in, which of
/// them need a copy, and the capacity the array must have.
#[derive(Debug, PartialEq)]
pub(crate) struct SlotPlan {
    /// The slot of each wanted path, in the order they were asked for.
    pub slot_of: Vec<usize>,
    /// The slots whose cube must be (re)copied this frame.
    pub copies: Vec<usize>,
    /// The capacity the array needs: the current one, or a grown one, in which case
    /// every slot is copied again.
    pub capacity: usize,
}

/// The cube path held in each array slot (`None` for a slot never filled or
/// invalidated).
#[derive(Debug, Default)]
pub(crate) struct SlotTable {
    paths: Vec<Option<String>>,
}

impl SlotTable {
    /// Place `wanted` (at most [`MAX_REFLECTION_PROBES`] distinct paths) into slots and
    /// record it: a path already resident keeps its slot, a new one takes the first
    /// slot no wanted path holds. When they do not fit, the table grows to the next
    /// power of two (capped at the budget) and every slot is re-copied.
    pub(crate) fn plan(&mut self, wanted: &[&str]) -> SlotPlan {
        let needed = wanted.len().min(MAX_REFLECTION_PROBES);
        let wanted = &wanted[..needed];
        if needed > self.paths.len() {
            let capacity = needed.next_power_of_two().min(MAX_REFLECTION_PROBES);
            self.paths = wanted.iter().map(|p| Some(p.to_string())).collect();
            self.paths.resize(capacity, None);
            return SlotPlan {
                slot_of: (0..needed).collect(),
                copies: (0..needed).collect(),
                capacity,
            };
        }
        let resident = |path: &str, paths: &[Option<String>]| {
            paths.iter().position(|p| p.as_deref() == Some(path))
        };
        let mut slot_of = vec![usize::MAX; needed];
        let mut taken = vec![false; self.paths.len()];
        for (k, path) in wanted.iter().enumerate() {
            if let Some(slot) = resident(path, &self.paths) {
                slot_of[k] = slot;
                taken[slot] = true;
            }
        }
        let mut copies = Vec::new();
        for (k, path) in wanted.iter().enumerate() {
            if slot_of[k] != usize::MAX {
                continue;
            }
            let slot = taken.iter().position(|t| !t).expect("needed <= capacity");
            taken[slot] = true;
            slot_of[k] = slot;
            self.paths[slot] = Some(path.to_string());
            copies.push(slot);
        }
        SlotPlan {
            slot_of,
            copies,
            capacity: self.paths.len(),
        }
    }

    /// Empty every slot whose path `stale` matches (a re-written cubemap, #1207), so
    /// the next plan copies it again.
    pub(crate) fn forget(&mut self, stale: impl Fn(&str) -> bool) {
        for slot in &mut self.paths {
            if slot.as_deref().is_some_and(&stale) {
                *slot = None;
            }
        }
    }

    /// Empty every slot: the array was rebuilt at another size.
    pub(crate) fn clear(&mut self) {
        self.paths.clear();
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod slots_tests;
