use rusty_physics::is_hittable;
use rusty_scene::{layer_in_mask, Scene};

/// A query's acceptance test (Unity's `layerMask` + `QueryTriggerInteraction`).
/// Rejected colliders are passed through during the traversal, never treated
/// as blockers.
#[derive(Debug, Clone, Copy)]
pub(in crate::api::physics) struct Filter {
    pub ignore: Option<u32>,
    pub mask: Option<u32>,
    pub triggers: bool,
}

impl Filter {
    /// A filter from a verb's optional Lua arguments. `query_triggers` absent
    /// (or nil) hits triggers, Unity's default (*Queries Hit Triggers* on).
    pub fn new(ignore: Option<u32>, mask: Option<u32>, query_triggers: Option<bool>) -> Self {
        Self {
            ignore,
            mask,
            triggers: query_triggers.unwrap_or(true),
        }
    }

    pub fn accepts(self, scene: &Scene, id: u32) -> bool {
        if Some(id) == self.ignore || !is_hittable(scene, id) {
            return false;
        }
        let world = &scene.world;
        // `physics::live` never makes a character's capsule a sensor, whatever `is_trigger` says.
        let trigger =
            world.collider(id).is_some_and(|c| c.is_trigger) && !world.has_character_controller(id);
        if !self.triggers && trigger {
            return false;
        }
        self.mask
            .is_none_or(|m| layer_in_mask(scene.world.layer(id), m))
    }
}

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
