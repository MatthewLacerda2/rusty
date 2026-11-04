//! src/navigation/agents/root_motion.rs — an agent moved by its Animator's root
//! motion (#1745).
//!
//! With *Apply Root Motion* on, a NavMeshAgent still plans its path and steers
//! (its `velocity` is the one it wants, for a script to feed the blend tree), but
//! it no longer moves itself: the root motion moves it, constrained to the
//! navmesh by the same walkable-cell slide its own steering uses (Unity's
//! `updatePosition = false` with the Animator driving the body). Root motion
//! drives it only while `AnimatorComponent::drives_root_motion` holds (the
//! animator running, its root bone in the skeleton); otherwise the agent moves
//! itself, so a stopped animator or a misspelled bone never freezes it.
//! Local avoidance sees it as a neighbour that doesn't dodge, moving at what
//! the root motion really moved it by last step.

use glam::Vec3;

use super::super::NavigationGraph;
use crate::scene::Scene;

/// The world velocity root motion really moved agent `id` at last step, when
/// root motion drives it (its Animator, see the module doc); `None` when the
/// agent moves itself.
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
