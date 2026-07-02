//! src/physics/trigger_events.rs — the per-tick trigger events `PhysicsWorld::step`
//! surfaces: which overlap pairs began this tick, which persist, and which ended
//! (#310). Edges are recovered by diffing this tick's sorted overlap set against
//! the previous tick's — rapier's `Started`/`Stopped` events carry the same
//! information, but the diff keeps the edge definition tied to exactly the set
//! `OnTrigger` already fires from, so enter/stay/exit can never disagree.

/// Trigger-overlap events for one physics tick. Every pair is `(low, high)`
/// entity ids and every list is sorted ascending — the same deterministic
/// ordering discipline as the rest of script dispatch.
#[derive(Debug, Default)]
pub struct TriggerEvents {
    /// Pairs overlapping this tick that were not overlapping last tick.
    pub entered: Vec<(u32, u32)>,
    /// Every pair overlapping this tick — the `OnTrigger` stay set, unchanged
    /// from the pre-#310 contract (it includes the pairs that just entered).
    pub stayed: Vec<(u32, u32)>,
    /// Pairs overlapping last tick that no longer overlap.
    pub exited: Vec<(u32, u32)>,
}

impl TriggerEvents {
    /// Diff the previous tick's overlap set against the current one. Both lists
    /// are sorted + deduped (`collect_triggers`' contract), so membership is a
    /// binary search and the outputs inherit the inputs' ordering.
    pub fn from_overlap_sets(prev: &[(u32, u32)], current: Vec<(u32, u32)>) -> Self {
        let entered = current
            .iter()
            .filter(|p| prev.binary_search(p).is_err())
            .copied()
            .collect();
        let exited = prev
            .iter()
            .filter(|p| current.binary_search(p).is_err())
            .copied()
            .collect();
        Self {
            entered,
            stayed: current,
            exited,
        }
    }

    /// True when the tick produced no trigger events at all (nothing to dispatch).
    pub fn is_empty(&self) -> bool {
        self.entered.is_empty() && self.stayed.is_empty() && self.exited.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::TriggerEvents;

    #[test]
    fn first_overlap_is_enter_and_stay() {
        let ev = TriggerEvents::from_overlap_sets(&[], vec![(1, 2)]);
        assert_eq!(ev.entered, vec![(1, 2)]);
        assert_eq!(ev.stayed, vec![(1, 2)]);
        assert!(ev.exited.is_empty());
    }

    #[test]
    fn persisting_overlap_is_stay_only() {
        let ev = TriggerEvents::from_overlap_sets(&[(1, 2)], vec![(1, 2)]);
        assert!(ev.entered.is_empty());
        assert_eq!(ev.stayed, vec![(1, 2)]);
        assert!(ev.exited.is_empty());
    }

    #[test]
    fn ended_overlap_is_exit_only() {
        let ev = TriggerEvents::from_overlap_sets(&[(1, 2)], Vec::new());
        assert!(ev.entered.is_empty());
        assert!(ev.stayed.is_empty());
        assert_eq!(ev.exited, vec![(1, 2)]);
        assert!(!ev.is_empty());
    }

    #[test]
    fn mixed_tick_keeps_lists_sorted_and_distinct() {
        // (1,2) persists, (3,4) enters, (5,6) exits.
        let ev = TriggerEvents::from_overlap_sets(&[(1, 2), (5, 6)], vec![(1, 2), (3, 4)]);
        assert_eq!(ev.entered, vec![(3, 4)]);
        assert_eq!(ev.stayed, vec![(1, 2), (3, 4)]);
        assert_eq!(ev.exited, vec![(5, 6)]);
    }

    #[test]
    fn no_overlaps_is_empty() {
        assert!(TriggerEvents::from_overlap_sets(&[], Vec::new()).is_empty());
    }
}
