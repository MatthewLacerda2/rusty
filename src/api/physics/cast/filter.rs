//! src/api/physics/cast/filter.rs — what every spatial query lets through.
//!
//! One [`Filter`] (ignore id, layer mask, trigger flag) and one [`reach`] rule
//! for a cast's `max_distance` (#1720), shared by the line casts, the
//! penetration cast and the volume queries, so every `Physics.*` query honours
//! the same ignore / layer-mask / trigger rules (#91, #311).

use crate::physics::is_hittable;
use crate::scene::{layer_in_mask, Scene};

/// A query's acceptance test (Unity's `layerMask` + `QueryTriggerInteraction`).
/// Rejected colliders are passed through during the traversal, never treated
/// as blockers.
#[derive(Debug, Clone, Copy)]
pub(in crate::api::physics) struct Filter {
    /// The one entity to skip (e.g. the shooter).
    pub ignore: Option<u32>,
    /// Bit per layer; `None` accepts every layer.
    pub mask: Option<u32>,
    /// Whether triggers are hit.
    pub triggers: bool,
}

impl Filter {
    /// A filter from a verb's optional Lua arguments. `query_triggers` absent
    /// (or nil) hits triggers: every query here always did, and it is Unity's
    /// default (*Queries Hit Triggers* on).
    pub fn new(ignore: Option<u32>, mask: Option<u32>, query_triggers: Option<bool>) -> Self {
        Self {
            ignore,
            mask,
            triggers: query_triggers.unwrap_or(true),
        }
    }

    /// Whether the collider owned by `id` counts: not the ignored entity,
    /// hittable ([`is_hittable`]), not a trigger when triggers are off, and on
    /// a layer the mask names. The trigger flag read is the scene's
    /// `Collider.is_trigger`, which the live world's sensor flag follows.
    pub fn accepts(self, scene: &Scene, id: u32) -> bool {
        if Some(id) == self.ignore || !is_hittable(scene, id) {
            return false;
        }
        if !self.triggers && scene.world.collider(id).is_some_and(|c| c.is_trigger) {
            return false;
        }
        self.mask
            .is_none_or(|m| layer_in_mask(scene.world.layer(id), m))
    }
}

/// How far a cast reaches for a `max_distance` argument, or `None` when it
/// casts nothing. Absent (nil) or infinite means unlimited (`f32::MAX`); a
/// negative or NaN distance hits nothing, as Unity's PhysX casts reject it.
/// The reach is handed to rapier, so the traversal itself stops there.
pub(in crate::api::physics) fn reach(max_distance: Option<f32>) -> Option<f32> {
    match max_distance {
        None => Some(f32::MAX),
        Some(d) if d >= 0.0 => Some(d.min(f32::MAX)),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_is_unlimited_when_absent_and_nothing_when_negative_or_nan() {
        assert_eq!(reach(None), Some(f32::MAX));
        assert_eq!(reach(Some(f32::INFINITY)), Some(f32::MAX));
        assert_eq!(reach(Some(5.0)), Some(5.0));
        assert_eq!(reach(Some(0.0)), Some(0.0));
        assert_eq!(reach(Some(-1.0)), None);
        assert_eq!(reach(Some(f32::NAN)), None);
    }
}
