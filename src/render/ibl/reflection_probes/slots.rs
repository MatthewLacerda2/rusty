//! Which reflection probe sits in which layer of the probe cube array (#1206): a pure
//! slot table, GPU-free so its reuse and growth rules are unit-tested headlessly.
//!
//! A slot keeps its cube until another probe needs the layer, so a camera walking
//! through a level copies only the probes that come into range, never the whole set.
//! The array keeps [`SPARE_SLOTS`] more slots than it shades with (#1584), and a new
//! cube overwrites the slot used least recently: the cubes that just left range stay
//! in the array, so a camera pacing across the budget's edge copies nothing back and
//! reads nothing from disk.

/// The most reflection probes shaded in one frame: the array's layer budget and the
/// length of `LightingUniforms.refl_probes`. The frame keeps the probes whose boxes
/// are nearest the camera (its distance to each box, zero inside it, #1238), as many
/// as the scene's byte budget holds (#1584); an object whose probe is past them
/// reflects the sky. 32 cubes and the spares are 204 array layers (under wgpu's
/// default limit of 256); the budget, not this cap, bounds their memory.
pub(crate) const MAX_REFLECTION_PROBES: usize = 32;

/// The slots the array keeps past the ones it shades with, holding the cubes that
/// left range last. The frame counts them inside the budget (`frame.rs`).
pub(crate) const SPARE_SLOTS: usize = 2;

/// One frame's plan for the array: the slot each wanted cube lands in, which of
/// them need a copy, and the capacity the array must have.
#[derive(Debug, PartialEq)]
pub(crate) struct SlotPlan {
    /// The slot of each wanted path, in the order they were asked for.
    pub slot_of: Vec<usize>,
    /// The slots whose cube must be (re)copied this frame.
    pub copies: Vec<usize>,
    /// The capacity the array needs: the current one, or a resized one, in which
    /// case every wanted cube is copied again.
    pub capacity: usize,
}

/// The cube path held in each array slot (`None` for a slot never filled or
/// invalidated), and the plan that last wanted it.
#[derive(Debug, Default)]
pub(crate) struct SlotTable {
    paths: Vec<Option<String>>,
    used: Vec<u64>,
    plans: u64,
}

impl SlotTable {
    /// Place `wanted` (at most [`MAX_REFLECTION_PROBES`] distinct paths) into slots and
    /// record it: a path the array holds keeps its slot, a new one takes the free
    /// slot wanted least recently. The array holds what is wanted and
    /// [`SPARE_SLOTS`] more, within `room` (the slots the budget pays for, #1584):
    /// when `wanted` outgrows it, it holds more than `room`, or twice what it would
    /// be sized to now, it is resized and every wanted cube is copied again.
    pub(crate) fn plan(&mut self, wanted: &[&str], room: usize) -> SlotPlan {
        let needed = wanted.len().min(MAX_REFLECTION_PROBES);
        let wanted = &wanted[..needed];
        self.plans += 1;
        let len = self.paths.len();
        let fit = (needed + SPARE_SLOTS).min(room.max(needed));
        if needed > len || len > room.max(needed) || fit * 2 <= len {
            self.paths = wanted.iter().map(|p| Some(p.to_string())).collect();
            self.paths.resize(fit, None);
            self.used = vec![self.plans; fit];
            return SlotPlan {
                slot_of: (0..needed).collect(),
                copies: (0..needed).collect(),
                capacity: fit,
            };
        }
        let mut slot_of = vec![usize::MAX; needed];
        for (k, path) in wanted.iter().enumerate() {
            if let Some(slot) = self.paths.iter().position(|p| p.as_deref() == Some(path)) {
                slot_of[k] = slot;
                self.used[slot] = self.plans;
            }
        }
        let mut copies = Vec::new();
        for (k, path) in wanted.iter().enumerate() {
            if slot_of[k] != usize::MAX {
                continue;
            }
            let slot = (0..len)
                .filter(|&s| self.used[s] != self.plans)
                .min_by_key(|&s| (self.paths[s].is_some(), self.used[s]))
                .expect("needed <= capacity");
            self.used[slot] = self.plans;
            self.paths[slot] = Some(path.to_string());
            slot_of[k] = slot;
            copies.push(slot);
        }
        SlotPlan {
            slot_of,
            copies,
            capacity: len,
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

    /// Whether any slot holds a cube.
    pub(crate) fn holds_any(&self) -> bool {
        self.paths.iter().any(Option::is_some)
    }

    /// Empty every slot: the array was rebuilt at another size.
    pub(crate) fn clear(&mut self) {
        self.paths.clear();
        self.used.clear();
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod slots_tests;
