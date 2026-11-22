//! `raw` is the pre-erosion spans: a column's depend on that column alone, so a dirty rectangle's are rebuilt and spliced in.

use std::collections::VecDeque;

use super::super::{NavMeshSettings, NavigationGraph};
use super::inputs::BakeInputs;
use super::region::CellRect;

/// How many recent generations the change log remembers. An agent planned against
/// an older one re-plans unconditionally.
const LOG_LEN: usize = 64;

pub struct BakeState {
    pub(super) raw: NavigationGraph,
    pub(super) inputs: BakeInputs,
    pub(super) log: ChangeLog,
}

#[derive(Default)]
pub(super) struct ChangeLog(VecDeque<(u64, Option<Vec<CellRect>>)>);

impl ChangeLog {
    pub fn record(&mut self, generation: u64, changed: Option<Vec<CellRect>>) {
        if self.0.len() == LOG_LEN {
            self.0.pop_front();
        }
        self.0.push_back((generation, changed));
    }

    fn since(&self, generation: u64) -> Option<Vec<CellRect>> {
        let first = self.0.front()?.0;
        if first > generation.wrapping_add(1) {
            return None;
        }
        let mut out = Vec::new();
        for (g, changed) in &self.0 {
            if *g > generation {
                out.extend_from_slice(changed.as_deref()?);
            }
        }
        Some(out)
    }
}

impl NavigationGraph {
    pub(crate) fn invalidate_paths(&mut self) {
        self.bake_generation = self.bake_generation.wrapping_add(1);
        self.islands.keep(self.bake_generation);
        if let Some(st) = self.bake_state.as_mut() {
            st.log.record(self.bake_generation, None);
        }
    }

    pub fn changes_since(&self, generation: u64) -> Option<Vec<CellRect>> {
        self.bake_state.as_ref()?.log.since(generation)
    }

    pub(crate) fn baked_settings(&self) -> Option<&NavMeshSettings> {
        Some(&self.bake_state.as_ref()?.inputs.settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32) -> CellRect {
        CellRect {
            x0: x,
            z0: 0,
            x1: x,
            z1: 0,
        }
    }

    #[test]
    fn since_collects_later_changes_and_gives_up_on_full_bakes() {
        let mut log = ChangeLog::default();
        log.record(1, None);
        log.record(2, Some(vec![r(2)]));
        log.record(3, Some(vec![r(3), r(4)]));
        assert_eq!(log.since(1), Some(vec![r(2), r(3), r(4)]));
        assert_eq!(log.since(2), Some(vec![r(3), r(4)]));
        assert_eq!(log.since(3), Some(vec![]));
        assert_eq!(log.since(0), None, "a full bake since");
        let mut short = ChangeLog::default();
        short.record(10, Some(vec![r(0)]));
        assert_eq!(short.since(5), None, "the log does not reach back");
    }
}
