//! src/physics/trigger_events.rs — the per-tick trigger events `PhysicsWorld::step`
//! surfaces: which overlap pairs began this tick, which persist, and which ended
//! (#310). Edges are recovered by diffing this tick's sorted overlap set against
//! the previous tick's — rapier's `Started`/`Stopped` events carry the same
//! information, but the diff keeps the edge definition tied to exactly the set
//! `OnTrigger` already fires from, so enter/stay/exit can never disagree.
//!
//! `collect_overlap_pairs` gathers that current-tick set from rapier's narrow
//! phase; it lives here (next to the diff that consumes it) rather than in
//! `world.rs`, keeping the step lifecycle file focused and under the size cap.

use std::collections::HashMap;

use rapier3d::prelude::*;

use super::build::order_pair;

/// Gather the overlapping pairs that involve a trigger/sensor or static body,
/// as `(low, high)` entity-id tuples, sorted + deduped. Sensors land in rapier's
/// intersection graph; solid contacts in the contact graph (kept only when one
/// side is a trigger). This is the current-tick set `TriggerEvents` diffs.
pub(super) fn collect_overlap_pairs(
    narrow_phase: &NarrowPhase,
    collider_to_id: &HashMap<ColliderHandle, u32>,
    id_is_trigger: &HashMap<u32, bool>,
) -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    for (h1, h2, intersecting) in narrow_phase.intersection_pairs() {
        if !intersecting {
            continue;
        }
        if let (Some(&a), Some(&b)) = (collider_to_id.get(&h1), collider_to_id.get(&h2)) {
            pairs.push(order_pair(a, b));
        }
    }
    for contact in narrow_phase.contact_pairs() {
        if !contact.has_any_active_contact {
            continue;
        }
        let a = collider_to_id.get(&contact.collider1);
        let b = collider_to_id.get(&contact.collider2);
        if let (Some(&a), Some(&b)) = (a, b) {
            let trig_a = *id_is_trigger.get(&a).unwrap_or(&false);
            let trig_b = *id_is_trigger.get(&b).unwrap_or(&false);
            if trig_a || trig_b {
                pairs.push(order_pair(a, b));
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

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
