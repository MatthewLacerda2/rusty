//! src/navigation/bake/state.rs — what a baked graph keeps to rebake incrementally (#456).
//!
//! * `raw` — the **pre-erosion** spans (rasterised, headroom-filtered, carved). A
//!   column's raw spans depend on that column alone, so a dirty rectangle's are
//!   rebuilt and spliced in; erosion then re-reads its neighbourhood from here.
//! * `inputs` — what the last bake read, for the per-tick diff.
//! * `log` — which cells each recent bake generation changed, so an agent re-plans
//!   only when its path crosses one.

use std::collections::VecDeque;

use super::super::NavigationGraph;
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

/// Recent bake generations and the cells each changed (`None`: everything).
#[derive(Default)]
pub(super) struct ChangeLog(VecDeque<(u64, Option<Vec<CellRect>>)>);

impl ChangeLog {
    pub fn record(&mut self, generation: u64, changed: Option<Vec<CellRect>>) {
        if self.0.len() == LOG_LEN {
            self.0.pop_front();
        }
        self.0.push_back((generation, changed));
    }

    /// Every rectangle changed after `generation` up to now, or `None` when a full
    /// bake happened since or the log no longer reaches back that far.
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
    /// Make every cached agent path stale without rebaking (#460: an area cost
    /// changed): a new generation whose change log entry says "anything changed".
    pub(in crate::navigation) fn invalidate_paths(&mut self) {
        self.bake_generation = self.bake_generation.wrapping_add(1);
        if let Some(st) = self.bake_state.as_mut() {
            st.log.record(self.bake_generation, None);
        }
    }

    /// The cell rectangles every bake after `generation` changed, or `None` when
    /// that is unknown (a full bake, or too long ago): then anything may have.
    pub fn changes_since(&self, generation: u64) -> Option<Vec<CellRect>> {
        self.bake_state.as_ref()?.log.since(generation)
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
