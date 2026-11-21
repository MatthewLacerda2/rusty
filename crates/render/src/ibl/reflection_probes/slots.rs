pub(crate) const MAX_REFLECTION_PROBES: usize = 32;

pub(crate) const SPARE_SLOTS: usize = 2;

#[derive(Debug, PartialEq)]
pub(crate) struct SlotPlan {
    pub slot_of: Vec<usize>,
    pub copies: Vec<usize>,
    pub capacity: usize,
}

#[derive(Debug, Default)]
pub(crate) struct SlotTable {
    paths: Vec<Option<String>>,
    used: Vec<u64>,
    plans: u64,
}

impl SlotTable {
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

    pub(crate) fn forget(&mut self, stale: impl Fn(&str) -> bool) {
        for slot in &mut self.paths {
            if slot.as_deref().is_some_and(&stale) {
                *slot = None;
            }
        }
    }

    pub(crate) fn holds_any(&self) -> bool {
        self.paths.iter().any(Option::is_some)
    }

    pub(crate) fn clear(&mut self) {
        self.paths.clear();
        self.used.clear();
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod slots_tests;
