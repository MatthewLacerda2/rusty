use glam::Vec3;

use rusty_components::{NavMeshAgentComponent, MAX_AVOIDANCE_PRIORITY};
use rusty_scene::rules::sanitize::{
    at_least, finite, finite3_or, finite_f64_or, non_negative, MIN_EXTENT,
};

pub fn set_active(a: &mut NavMeshAgentComponent, active: bool) {
    a.active = active;
}

pub fn set_speed(a: &mut NavMeshAgentComponent, speed: f32) {
    a.speed = non_negative(speed);
}

pub fn set_acceleration(a: &mut NavMeshAgentComponent, acceleration: f32) {
    a.acceleration = non_negative(acceleration);
}

pub fn set_stopping_distance(a: &mut NavMeshAgentComponent, stopping_distance: f32) {
    a.stopping_distance = non_negative(stopping_distance);
}

pub fn set_update_rotation(a: &mut NavMeshAgentComponent, update: bool) {
    a.update_rotation = update;
}

/// Set the agent's top yaw rate, degrees/second (Unity's `angularSpeed`).
pub fn set_angular_speed(a: &mut NavMeshAgentComponent, angular_speed: f32) {
    a.angular_speed = non_negative(angular_speed);
}

/// Set how fast the agent's yaw rate ramps up and down, degrees/second².
pub fn set_angular_acceleration(a: &mut NavMeshAgentComponent, angular_acceleration: f32) {
    a.angular_acceleration = non_negative(angular_acceleration);
}

pub fn set_radius(a: &mut NavMeshAgentComponent, radius: f32) {
    a.radius = at_least(radius, MIN_EXTENT);
}

pub fn set_base_offset(a: &mut NavMeshAgentComponent, base_offset: f32) {
    a.base_offset = finite(base_offset);
}

pub fn set_target(a: &mut NavMeshAgentComponent, target: Vec3) {
    a.target = finite3_or(target, a.target);
}

/// `finite_f64_or` first: a bare `as u8` would turn NaN into 0, the most important priority.
pub fn set_avoidance_priority(a: &mut NavMeshAgentComponent, priority: f64) {
    let priority = finite_f64_or(priority, f64::from(a.avoidance_priority));
    a.avoidance_priority = priority.clamp(0.0, f64::from(MAX_AVOIDANCE_PRIORITY)) as u8;
}

pub fn set_avoidance_enabled(a: &mut NavMeshAgentComponent, enabled: bool) {
    a.avoidance_enabled = enabled;
}

pub fn set_auto_traverse_off_mesh_link(a: &mut NavMeshAgentComponent, auto: bool) {
    a.auto_traverse_off_mesh_link = auto;
}

pub fn set_area_mask(a: &mut NavMeshAgentComponent, mask: u32) {
    a.set_area_mask(mask);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_components::NavMeshAgentComponent;
    use rusty_scene::Scene;

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
        set_update_rotation(&mut e, false);
        set_angular_speed(&mut e, 90.0);
        set_angular_acceleration(&mut e, 360.0);
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
        assert!(!a.update_rotation);
        assert_eq!(a.angular_speed, 90.0);
        assert_eq!(a.angular_acceleration, 360.0);
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
