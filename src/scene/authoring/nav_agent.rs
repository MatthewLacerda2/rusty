//! src/scene/authoring/nav_agent.rs — Shared nav-agent-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `NavMeshAgentComponent` field by field: the `active` flag, the motion tuning
//! (`speed` / `acceleration` / `stopping_distance`), the footprint `radius`, and the
//! `target` destination.
//!
//! BOTH the editor's NavMesh Agent card and the Lua `NavMeshAgent.*` setters route
//! through these, so the egui panel and the binding share one write (#287). The ops
//! are plain field sets (no clamps) so the binding's behaviour is byte-identical to
//! before; the card's drag-range clamps are a UI affordance applied to the widget's
//! local value before the op is called, so the editor still bounds its inputs.
//!
//! Allowed deps: components (the `NavMeshAgentComponent` data). Pure.

use glam::Vec3;

use crate::components::Entity;

/// Set the agent's `active` flag.
pub fn set_active(entity: &mut Entity, active: bool) {
    if let Some(a) = &mut entity.nav_agent {
        a.active = active;
    }
}

/// Set the agent's max speed.
pub fn set_speed(entity: &mut Entity, speed: f32) {
    if let Some(a) = &mut entity.nav_agent {
        a.speed = speed;
    }
}

/// Set the agent's acceleration.
pub fn set_acceleration(entity: &mut Entity, acceleration: f32) {
    if let Some(a) = &mut entity.nav_agent {
        a.acceleration = acceleration;
    }
}

/// Set the agent's stopping distance.
pub fn set_stopping_distance(entity: &mut Entity, stopping_distance: f32) {
    if let Some(a) = &mut entity.nav_agent {
        a.stopping_distance = stopping_distance;
    }
}

/// Set the agent's footprint radius.
pub fn set_radius(entity: &mut Entity, radius: f32) {
    if let Some(a) = &mut entity.nav_agent {
        a.radius = radius;
    }
}

/// Set the agent's destination target.
pub fn set_target(entity: &mut Entity, target: Vec3) {
    if let Some(a) = &mut entity.nav_agent {
        a.target = target;
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
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.nav_agent = Some(NavMeshAgentComponent::default());
        }
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_agent();
        let mut e = scene.get_entity_mut(id).unwrap();
        set_active(&mut e, true);
        set_speed(&mut e, 3.5);
        set_acceleration(&mut e, 8.0);
        set_stopping_distance(&mut e, 0.5);
        set_radius(&mut e, 0.4);
        set_target(&mut e, Vec3::new(1.0, 0.0, 2.0));
        let a = e.nav_agent.as_ref().unwrap();
        assert!(a.active);
        assert_eq!(a.speed, 3.5);
        assert_eq!(a.acceleration, 8.0);
        assert_eq!(a.stopping_distance, 0.5);
        assert_eq!(a.radius, 0.4);
        assert_eq!(a.target, Vec3::new(1.0, 0.0, 2.0));
    }
}
