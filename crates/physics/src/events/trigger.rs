use rapier3d::prelude::*;

use crate::query::is_live;
use crate::shape::build::order_pair;
use crate::world::PhysicsWorld;

impl PhysicsWorld {
    /// rapier's narrow phase runs at the *start* of a step, so its sensor pairs lag a tick;
    /// each live sensor instead asks the query tree what it overlaps now.
    /// A static trigger reports only colliders whose body is not fixed; unlike contacts, triggers
    /// still pair kinematic with static and kinematic with kinematic, so a static zone sees a
    /// CharacterController or a kinematic enemy walk in. Do not align this with
    /// `assemble::ACTIVE_TYPES`.
    pub(crate) fn overlap_pairs(&self) -> Vec<(u32, u32)> {
        let fixed = |c: &Collider| {
            let body = c.parent().and_then(|h| self.bodies.get(h));
            body.is_none_or(|b| b.is_fixed())
        };
        let mut pairs = Vec::new();
        for (handle, sensor) in self.colliders.iter() {
            let Some(&id) = self.collider_to_id.get(&handle) else {
                continue;
            };
            if !sensor.is_sensor() || !is_live(&self.bodies, sensor) {
                continue;
            }
            let sensor_fixed = fixed(sensor);
            let live = |_: ColliderHandle, c: &Collider| {
                is_live(&self.bodies, c) && !(sensor_fixed && fixed(c))
            };
            let mut filter = QueryFilter::default()
                .groups(sensor.collision_groups())
                .exclude_collider(handle);
            filter.predicate = Some(&live);
            if let Some(body) = sensor.parent() {
                filter = filter.exclude_rigid_body(body);
            }
            let queries = self.queries(filter);
            let hits = queries.intersect_shape(*sensor.position(), sensor.shape());
            pairs.extend(hits.filter_map(|(other, _)| {
                let &other = self.collider_to_id.get(&other)?;
                Some(order_pair(id, other))
            }));
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }
}

#[derive(Debug, Default)]
pub struct TriggerEvents {
    pub entered: Vec<(u32, u32)>,
    pub stayed: Vec<(u32, u32)>,
    pub exited: Vec<(u32, u32)>,
}

impl TriggerEvents {
    /// Both sets sorted and deduped, as `overlap_pairs` returns them: membership is a binary
    /// search.
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
