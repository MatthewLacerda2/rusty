//! src/navigation/agents/root_motion.rs — an agent moved by its Animator's root
//! motion (#1745).
//!
//! With *Apply Root Motion* on, a NavMeshAgent still plans its path and steers
//! (its `velocity` is the one it wants, for a script to feed the blend tree), but
//! it no longer moves itself: the root motion moves it, constrained to the
//! navmesh by the same walkable-cell slide its own steering uses (Unity's
//! `updatePosition = false` with the Animator driving the body).

use glam::Vec3;

use super::super::NavigationGraph;
use crate::scene::Scene;

impl NavigationGraph {
    /// Move agent `id` by the world-space displacement `motion`, sliding along
    /// the navmesh as its steering would: only onto walkable spans linked to the
    /// one it stands on, then onto that floor's height. The agent's own velocity
    /// is left as it was. Nothing moves without an agent.
    pub fn move_agent(&self, scene: &mut Scene, id: u32, motion: Vec3) {
        let Some(mut agent) = scene.world.take_nav_agent(id) else {
            return;
        };
        let velocity = agent.velocity;
        // One "second" of a velocity equal to the displacement.
        agent.velocity = Vec3::new(motion.x, 0.0, motion.z);
        let to = self.slide(&mut agent, scene.world.world_position(id), 1.0);
        agent.velocity = velocity;
        scene.world.set_nav_agent(id, Some(agent));
        scene.world.set_world_position(id, to);
    }
}
