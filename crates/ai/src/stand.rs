//! crates/ai/src/stand.rs — where an entity **stands** (#1892, #1900): its feet, not
//! its Transform, which for a character is its middle. The environment query's
//! contexts and every perception sense's last-known position read it, so a
//! trace's eye height and a distance mean the same for every kind of body.

use glam::Vec3;

use rusty_scene::Scene;

/// Entity `id`'s feet: a `NavMeshAgent`'s, else the bottom of a
/// `CharacterController`'s capsule, else the bottom of its collider, else its
/// Transform. Only the height moves; x and z are the Transform's.
pub fn feet(scene: &Scene, id: u32) -> Vec3 {
    let w = &scene.world;
    let at = w.world_position(id);
    let feet_y = if let Some(agent) = w.nav_agent(id) {
        agent.feet(at).y
    } else if let (Some(cc), Some(pose)) = (w.character_controller(id), w.world_pose(id)) {
        let center = pose.pos + pose.rot * (cc.center * pose.scale);
        center.y - 0.5 * cc.height.max(2.0 * cc.radius) * pose.scale.y.abs()
    } else if let Some((min, _)) = w.collider_bounds(id) {
        min.y
    } else {
        at.y
    };
    at.with_y(feet_y)
}
