//! src/scene/authoring/nav_agent.rs — Shared nav-agent-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `NavMeshAgentComponent` field by field: the `active` flag, the motion tuning
//! (`speed` / `acceleration` / `stopping_distance`), the footprint `radius` and
//! `base_offset`, the
//! `target` destination, and the local-avoidance settings (#463).
//!
//! BOTH the editor's NavMesh Agent card and the Lua `NavMeshAgent.*` setters route
//! through these, so the egui panel and the binding share one write (#287). The ops
//! are plain field sets (no clamps) so the binding's behaviour is byte-identical to
//! before; the card's drag-range clamps are a UI affordance applied to the widget's
//! local value before the op is called, so the editor still bounds its inputs.
//!
//! Allowed deps: components (the `NavMeshAgentComponent` data). Pure.

use glam::Vec3;

use crate::components::{NavMeshAgentComponent, MAX_AVOIDANCE_PRIORITY};

/// Set the agent's `active` flag.
pub fn set_active(a: &mut NavMeshAgentComponent, active: bool) {
    a.active = active;
}

/// Set the agent's max speed.
pub fn set_speed(a: &mut NavMeshAgentComponent, speed: f32) {
    a.speed = speed;
}

/// Set the agent's acceleration.
pub fn set_acceleration(a: &mut NavMeshAgentComponent, acceleration: f32) {
    a.acceleration = acceleration;
}

/// Set the agent's stopping distance.
pub fn set_stopping_distance(a: &mut NavMeshAgentComponent, stopping_distance: f32) {
    a.stopping_distance = stopping_distance;
}

/// Set the agent's footprint radius.
pub fn set_radius(a: &mut NavMeshAgentComponent, radius: f32) {
    a.radius = radius;
}

/// Set how far the entity's origin sits above the agent's feet (#666, Unity's
/// `baseOffset`).
pub fn set_base_offset(a: &mut NavMeshAgentComponent, base_offset: f32) {
    a.base_offset = base_offset;
}

/// Set the agent's destination target.
pub fn set_target(a: &mut NavMeshAgentComponent, target: Vec3) {
    a.target = target;
}

/// Set the agent's avoidance priority (Unity's `avoidancePriority`: lower is more
/// important). The one clamp among these ops: the field is `u8` and the range is
/// 0–99, so anything outside lands on the nearest end (fractions truncate).
pub fn set_avoidance_priority(a: &mut NavMeshAgentComponent, priority: f64) {
    a.avoidance_priority = priority.clamp(0.0, f64::from(MAX_AVOIDANCE_PRIORITY)) as u8;
}

/// Set whether the agent steers around other agents.
pub fn set_avoidance_enabled(a: &mut NavMeshAgentComponent, enabled: bool) {
    a.avoidance_enabled = enabled;
}

/// Set whether the engine moves the agent across off-mesh links itself (#462,
/// Unity's `autoTraverseOffMeshLink`), or leaves it on the link for a script.
pub fn set_auto_traverse_off_mesh_link(a: &mut NavMeshAgentComponent, auto: bool) {
    a.auto_traverse_off_mesh_link = auto;
}

/// Set the areas the agent may enter (#460, Unity's `areaMask`). Its cached path was
/// planned under the old mask, so it is dropped and the agent re-plans next tick.
pub fn set_area_mask(a: &mut NavMeshAgentComponent, mask: u32) {
    if a.area_mask != mask {
        a.area_mask = mask;
        a.cached_path.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::NavMeshAgentComponent;
    use crate::scene::Scene;

    fn scene_with_agent() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Agent".to_string());
        scene
            .world
            .set_nav_agent(id, Some(NavMeshAgentComponent::default()));
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_agent();
        let mut e = scene.world.nav_agent_mut(id).unwrap();
        set_active(&mut e, true);
        set_speed(&mut e, 3.5);
        set_acceleration(&mut e, 8.0);
        set_stopping_distance(&mut e, 0.5);
        set_radius(&mut e, 0.4);
        set_base_offset(&mut e, 1.0);
        set_target(&mut e, Vec3::new(1.0, 0.0, 2.0));
        set_avoidance_priority(&mut e, 12.0);
        set_avoidance_enabled(&mut e, false);
        set_auto_traverse_off_mesh_link(&mut e, false);
        e.cached_path = vec![Vec3::ONE];
        set_area_mask(&mut e, 0b101);
        assert!(e.cached_path.is_empty(), "a new mask re-plans");
        let a = &*e;
        assert_eq!(a.area_mask, 0b101);
        assert!(a.active);
        assert_eq!(a.speed, 3.5);
        assert_eq!(a.acceleration, 8.0);
        assert_eq!(a.stopping_distance, 0.5);
        assert_eq!(a.radius, 0.4);
        assert_eq!(a.base_offset, 1.0);
        assert_eq!(a.target, Vec3::new(1.0, 0.0, 2.0));
        assert_eq!(a.avoidance_priority, 12);
        assert!(!a.avoidance_enabled);
        assert!(!a.auto_traverse_off_mesh_link);
    }

    #[test]
    fn avoidance_priority_clamps_to_unity_range() {
        let mut a = NavMeshAgentComponent::default();
        set_avoidance_priority(&mut a, 250.0);
        assert_eq!(a.avoidance_priority, 99);
        set_avoidance_priority(&mut a, -3.0);
        assert_eq!(a.avoidance_priority, 0);
        set_avoidance_priority(&mut a, 42.9);
        assert_eq!(a.avoidance_priority, 42);
    }
}
