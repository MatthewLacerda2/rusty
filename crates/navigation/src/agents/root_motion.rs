//! With *Apply Root Motion* on, the agent still plans and steers but the Animator's root
//! motion moves it (Unity's `updatePosition = false`), sliding on the navmesh. Only while
//! `drives_root_motion` holds; otherwise it moves itself, so a stopped animator or a
//! misspelled bone never freezes it.

use glam::Vec3;

use super::super::NavigationGraph;
use rusty_scene::Scene;

pub(super) fn root_moved(scene: &Scene, id: u32, delta_time: f32) -> Option<Vec3> {
    let anim = scene.world.animator(id)?;
    let mesh = scene.world.mesh(id);
    let skin = mesh.as_deref().and_then(|m| m.skin.as_ref());
    anim.drives_root_motion(skin)
        .then(|| match delta_time > 0.0 {
            true => anim.root_motion.moved / delta_time,
            false => Vec3::ZERO,
        })
}

impl NavigationGraph {
    pub fn move_agent(&self, scene: &mut Scene, id: u32, motion: Vec3) {
        let body = scene.world.world_position(id);
        let to = {
            let Some(mut agent) = scene.world.nav_agent_mut(id) else {
                return;
            };
            let velocity = agent.velocity;
            // One "second" of a velocity equal to the displacement.
            agent.velocity = Vec3::new(motion.x, 0.0, motion.z);
            let to = self.slide(&mut agent, body, 1.0);
            agent.velocity = velocity;
            to
        };
        scene.world.set_world_position(id, to);
    }
}
