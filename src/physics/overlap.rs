//! src/physics/overlap.rs — gather this tick's raw overlap pairs from rapier.
//!
//! Pure read of the narrow phase's intersection/contact graphs into the legacy
//! `Vec<(u32, u32)>` trigger-pair contract `TriggerEvents::from_overlap_sets`
//! diffs each tick. Kept out of `world.rs` so that file stays focused on the
//! step/sync lifecycle (and under the size cap).

use std::collections::HashMap;

use rapier3d::prelude::{ColliderHandle, NarrowPhase};

use super::build::order_pair;

/// Gather overlapping pairs that involve a trigger/sensor or static body.
/// Sensors land in the intersection graph; solid contacts in the contact graph.
pub(super) fn collect_triggers(
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
