//! The per-frame agent steering tick: preferred velocity toward the next waypoint,
//! local avoidance between agents (#463) and around non-carving obstacles (#456),
//! then the walkable-cell slide.

use glam::{Vec2, Vec3};

use super::super::avoidance::{self, AvoidanceAgent};
use super::super::obstacle::avoidance_obstacles;
use super::super::NavigationGraph;
use super::link::auto_traverse;
use super::state::is_at_target;
use crate::scene::{NavMeshAgentComponent, Scene};

/// An agent that took part in this frame's tick, carried from the steering pass
/// to the integration pass.
struct Ticked {
    id: u32,
    position: Vec3,
    /// Outside its stopping distance, so it moves this frame.
    steering: bool,
}

fn xz(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

impl NavigationGraph {
    /// Steers and updates positions of active NavMesh agents in the scene, in
    /// three passes: each agent computes its preferred velocity, ORCA local avoidance (#463)
    /// bends those so agents steer around each other, and each agent integrates
    /// the result, constrained to walkable cells by the 2D slide. Agents are
    /// visited in the ECS's stable order, so the tick stays deterministic.
    pub fn tick_nav_agents(&self, scene: &mut Scene, delta_time: f32) {
        let mut ticked = Vec::new();
        let mut inputs = Vec::new();
        for id in scene.world.ids_with_nav_agent() {
            if !scene.world.is_active(id) {
                continue;
            }
            // Take the agent out, steer, write it back — the same idiom as the
            // particle tick, so the transform borrow never aliases the agent's.
            let Some(mut agent) = scene.world.take_nav_agent(id) else {
                continue;
            };
            let current_pos = scene.world.transform(id).map(|t| t.position);
            if let (true, Some(position)) = (agent.active, current_pos) {
                if agent.off_mesh_link.is_some() {
                    self.on_link(scene, id, agent, position, delta_time);
                    continue;
                }
                // Re-plan first, so arrival is measured against this target's path —
                // unless there is nothing to plan: no path, already at the target
                // (a `ResetPath` agent plans nothing until it gets a new one).
                if !(agent.cached_path.is_empty() && is_at_target(&agent, position)) {
                    let feet = agent.feet(position);
                    self.refresh_path(&mut agent, feet);
                }
                let steering = !agent.cached_path.is_empty() && !is_at_target(&agent, position);
                let moved_with = if steering { agent.velocity } else { Vec3::ZERO };
                if steering {
                    self.accelerate_toward_waypoint(&mut agent, position, delta_time);
                } else {
                    decelerate(&mut agent, delta_time);
                }
                inputs.push(avoidance_input(&agent, position, moved_with, steering));
                ticked.push(Ticked {
                    id,
                    position,
                    steering,
                });
            }
            scene.world.set_nav_agent(id, Some(agent));
        }

        // Non-carving obstacles join after the agents, as neighbours that never
        // dodge (#456); `ticked` stops the zip before them.
        inputs.extend(avoidance_obstacles(scene));
        let velocities = avoidance::solve(&inputs, delta_time);
        for (t, v) in ticked.iter().zip(velocities) {
            if t.steering {
                self.integrate(scene, t, Vec3::new(v.x, 0.0, v.y), delta_time);
            }
            // Keep entity's collider bounds aligned with transform positioning
            // (local matrix only — the legacy path never resolved the parent here).
            let local = scene.world.transform(t.id).map(|t| t.to_matrix());
            if let (Some(local), Some(mut col)) = (local, scene.world.collider_mut(t.id)) {
                let (min, max) = col.calculate_world_aabb(local);
                col.aabb_min = min;
                col.aabb_max = max;
            }
        }
    }

    /// Accelerate the agent's velocity toward its next waypoint — its preferred
    /// velocity this frame. Planar (XZ): horizontal speed is unaffected by climbing.
    fn accelerate_toward_waypoint(
        &self,
        agent: &mut NavMeshAgentComponent,
        current_pos: Vec3,
        delta_time: f32,
    ) {
        // Query the next waypoint from the agent's cached path,
        // re-planning with A* only when the cache is invalid (#126).
        let feet = agent.feet(current_pos);
        let next_step = self.cached_next_step(agent, feet);
        if agent.off_mesh_link.is_some() {
            return; // reached a link: it stands still until the link is crossed
        }
        // Steer on the XZ plane so a tall step doesn't inflate the move distance.
        let mut to_next_dir = next_step - current_pos;
        to_next_dir.y = 0.0;
        let to_next_dir = to_next_dir.normalize_or_zero();

        // Accelerate steering velocity (kept planar; y is snapped, not integrated).
        let desired_vel = to_next_dir * agent.speed;
        let diff_vel = desired_vel - agent.velocity;
        agent.velocity += diff_vel * (agent.acceleration * delta_time).min(1.0);
        agent.velocity.y = 0.0;
    }

    /// An agent on an off-mesh link (#462) neither steers nor avoids: the engine
    /// carries it across when it auto-traverses, else it waits for its script.
    fn on_link(
        &self,
        scene: &mut Scene,
        id: u32,
        mut agent: NavMeshAgentComponent,
        position: Vec3,
        delta_time: f32,
    ) {
        let moved = agent
            .auto_traverse_off_mesh_link
            .then(|| auto_traverse(&mut agent, position, delta_time));
        scene.world.set_nav_agent(id, Some(agent));
        if let (Some(p), Some(mut t)) = (moved, scene.world.transform_mut(id)) {
            t.position = p;
        }
    }

    /// Move a steering agent by its avoided `velocity` and write the new position.
    fn integrate(&self, scene: &mut Scene, t: &Ticked, velocity: Vec3, delta_time: f32) {
        let Some(mut agent) = scene.world.take_nav_agent(t.id) else {
            return;
        };
        agent.velocity = velocity;
        let new_pos = self.slide(&mut agent, t.position, delta_time);
        if let Some(mut tr) = scene.world.transform_mut(t.id) {
            tr.position = new_pos;
        }
        scene.world.set_nav_agent(t.id, Some(agent));
    }

    /// Return the slide-constrained position after one step of `agent.velocity`.
    /// Each axis moves only onto a span linked to the agent's current one (walkable,
    /// within the step/slope limits, enough headroom), so agents climb ramps and
    /// stairs but never slide up a wall face (#130). The agent then stands on the
    /// span it reached — the floor it walked onto, not whichever floor of that
    /// column is highest (#454) — with its Transform `base_offset` above it (#666).
    /// Off the navmesh it does not move. `body` is the agent's Transform position.
    fn slide(&self, agent: &mut NavMeshAgentComponent, body: Vec3, delta_time: f32) -> Vec3 {
        let current_pos = agent.feet(body);
        let Some(from) = self.span_under(current_pos) else {
            agent.velocity = Vec3::ZERO;
            return body;
        };
        let mut final_pos = current_pos;
        let mut on = from;

        let proposed_x = current_pos + Vec3::new(agent.velocity.x * delta_time, 0.0, 0.0);
        let (gx, gz) = self.world_to_grid(proposed_x);
        match self.link_to(from, gx, gz) {
            Some(s) => {
                final_pos.x = proposed_x.x;
                on = s;
            }
            None => agent.velocity.x = 0.0,
        }

        let proposed_z = final_pos + Vec3::new(0.0, 0.0, agent.velocity.z * delta_time);
        let (gx, gz) = self.world_to_grid(proposed_z);
        match self.link_to(from, gx, gz) {
            Some(s) => {
                final_pos.z = proposed_z.z;
                on = s;
            }
            None => agent.velocity.z = 0.0,
        }

        final_pos.y = self.spans[on.index as usize].y;
        agent.body(final_pos)
    }
}

/// The agent as local avoidance sees it: where it is, the velocity it moved with,
/// the one it wants, and whether it dodges this frame.
fn avoidance_input(
    agent: &NavMeshAgentComponent,
    position: Vec3,
    moved_with: Vec3,
    steering: bool,
) -> AvoidanceAgent {
    AvoidanceAgent {
        position: xz(position),
        velocity: xz(moved_with),
        preferred: xz(agent.velocity),
        radius: agent.radius,
        max_speed: agent.speed,
        priority: agent.avoidance_priority,
        solves: steering && agent.avoidance_enabled,
    }
}

/// Decelerate to zero velocity when inside the stopping distance.
fn decelerate(agent: &mut NavMeshAgentComponent, delta_time: f32) {
    agent.velocity -= agent.velocity * (agent.acceleration * delta_time).min(1.0);
    if agent.velocity.length_squared() < 0.001 {
        agent.velocity = Vec3::ZERO;
    }
}
