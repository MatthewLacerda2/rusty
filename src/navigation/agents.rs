use super::NavigationGraph;
use crate::scene::{NavMeshAgentComponent, Scene};
use glam::Vec3;

/// Squared world-distance the target may drift before a cached agent path is
/// considered stale and re-planned. Keeps tiny jitter from re-running A*.
const REPLAN_TARGET_EPSILON_SQ: f32 = 0.25;
/// Upper bound (in fixed-update frames) on how long a cached path may live
/// before a forced re-plan. Frame-count based, never wall-clock, so the sim
/// stays a pure function of (seed, inputs, dt).
const MAX_PATH_AGE_FRAMES: u32 = 60;
/// How close (world units, XZ plane) the agent must get to a waypoint before
/// the cursor advances to the next one.
const WAYPOINT_REACHED_DISTANCE: f32 = 0.5;

impl NavigationGraph {
    /// Whether `agent`'s cached path must be discarded and re-planned. Re-plan
    /// when there is no path, when the cursor has run off the end, when the
    /// navmesh was rebaked since planning, when the target drifted past the
    /// epsilon, when the path has aged out, or when the next waypoint is no
    /// longer walkable.
    fn path_cache_invalid(&self, agent: &NavMeshAgentComponent) -> bool {
        if agent.cached_path.is_empty() || agent.path_cursor >= agent.cached_path.len() {
            return true;
        }
        if agent.path_generation != self.bake_generation {
            return true;
        }
        if (agent.target - agent.planned_target).length_squared() > REPLAN_TARGET_EPSILON_SQ {
            return true;
        }
        if agent.frames_since_replan >= MAX_PATH_AGE_FRAMES {
            return true;
        }
        let wp = agent.cached_path[agent.path_cursor];
        let (gx, gz) = self.world_to_grid(wp);
        !self.is_walkable(gx, gz)
    }

    /// Runs A* once from the agent's current cell to its target and stores the
    /// resulting waypoints on the agent, resetting the cursor and bookkeeping.
    /// The start cell is dropped (the agent is already there) and the literal
    /// target is appended so the agent finishes on the goal, not the goal's cell
    /// center. A failed search still yields a one-waypoint beeline to the target.
    fn plan_agent_path(&self, agent: &mut NavMeshAgentComponent, current_pos: Vec3) {
        let (sx, sz) = self.world_to_grid(current_pos);
        let (tx, tz) = self.world_to_grid(agent.target);
        let mut waypoints = Vec::new();
        if let Some(path) = self.find_path(sx, sz, tx, tz) {
            for &(gx, gz) in path.iter().skip(1) {
                waypoints.push(self.grid_to_world(gx, gz));
            }
        }
        waypoints.push(agent.target);
        agent.cached_path = waypoints;
        agent.path_cursor = 0;
        agent.planned_target = agent.target;
        agent.path_generation = self.bake_generation;
        agent.frames_since_replan = 0;
    }

    /// Returns the world position the agent should steer toward this frame,
    /// using the cached path (re-planning only when invalid) and advancing the
    /// cursor past any waypoints already reached. Y is taken from the agent so
    /// horizontal steering matches the legacy `get_next_path_step` behaviour.
    fn cached_next_step(&self, agent: &mut NavMeshAgentComponent, current_pos: Vec3) -> Vec3 {
        if self.path_cache_invalid(agent) {
            self.plan_agent_path(agent, current_pos);
        }
        while agent.path_cursor < agent.cached_path.len() {
            let wp = agent.cached_path[agent.path_cursor];
            let dx = wp.x - current_pos.x;
            let dz = wp.z - current_pos.z;
            if (dx * dx + dz * dz).sqrt() <= WAYPOINT_REACHED_DISTANCE {
                agent.path_cursor += 1;
            } else {
                break;
            }
        }
        agent.frames_since_replan = agent.frames_since_replan.saturating_add(1);
        match agent.cached_path.get(agent.path_cursor) {
            Some(wp) => Vec3::new(wp.x, current_pos.y, wp.z),
            None => agent.target,
        }
    }

    /// Steers and updates positions of active NavMesh agents in the scene,
    /// constraining them strictly to walkable NavMesh cells using a 2D sliding projection check.
    #[allow(clippy::too_many_lines)]
    pub fn tick_nav_agents(&self, scene: &mut Scene, delta_time: f32) {
        for id in scene.entity_ids() {
            let mut entity_guard = match scene.get_entity_mut(id) {
                Some(e) => e,
                None => continue,
            };
            let entity: &mut crate::scene::Entity = &mut entity_guard;
            if !entity.active {
                continue;
            }

            if let Some(agent) = &mut entity.nav_agent {
                if !agent.active {
                    continue;
                }

                let current_pos = entity.transform.position;
                let to_target = agent.target - current_pos;
                let dist = to_target.length();

                if dist > agent.stopping_distance {
                    // Query the next waypoint from the agent's cached path,
                    // re-planning with A* only when the cache is invalid (#126).
                    let next_step = self.cached_next_step(agent, current_pos);
                    let to_next = next_step - current_pos;
                    let to_next_dir = to_next.normalize_or_zero();

                    // Accelerate steering velocity
                    let desired_vel = to_next_dir * agent.speed;
                    let diff_vel = desired_vel - agent.velocity;
                    agent.velocity += diff_vel * (agent.acceleration * delta_time).min(1.0);

                    // Proposed step
                    let proposed_pos_x =
                        current_pos + Vec3::new(agent.velocity.x * delta_time, 0.0, 0.0);
                    let mut final_pos = current_pos;

                    // Test X movement slide
                    let (gx, gz) = self.world_to_grid(proposed_pos_x);
                    if self.is_walkable(gx, gz) {
                        final_pos.x = proposed_pos_x.x;
                    } else {
                        agent.velocity.x = 0.0;
                    }

                    // Test Z movement slide
                    let proposed_pos_z =
                        final_pos + Vec3::new(0.0, 0.0, agent.velocity.z * delta_time);
                    let (gx, gz) = self.world_to_grid(proposed_pos_z);
                    if self.is_walkable(gx, gz) {
                        final_pos.z = proposed_pos_z.z;
                    } else {
                        agent.velocity.z = 0.0;
                    }

                    // Preserve original Y position
                    final_pos.y = current_pos.y;

                    // Apply the constrained position to entity transform
                    entity.transform.position = final_pos;
                } else {
                    // Decelerate to zero velocity when close
                    agent.velocity -= agent.velocity * (agent.acceleration * delta_time).min(1.0);
                    if agent.velocity.length_squared() < 0.001 {
                        agent.velocity = Vec3::ZERO;
                    }
                }

                // Keep entity's collider bounds aligned with transform positioning
                entity.update_collider(None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    fn open_graph() -> NavigationGraph {
        // 21x21 fully-walkable grid (no colliders baked).
        NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0)
    }

    fn test_agent(target: Vec3) -> NavMeshAgentComponent {
        NavMeshAgentComponent {
            active: true,
            radius: 0.5,
            target,
            speed: 5.0,
            acceleration: 10.0,
            stopping_distance: 0.5,
            velocity: Vec3::ZERO,
            ..Default::default()
        }
    }

    #[test]
    fn fresh_agent_plans_then_reuses_until_target_moves() {
        let graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 10.0));
        let start = Vec3::new(1.0, 0.0, 1.0);

        // No path yet → invalid → plan it.
        assert!(graph.path_cache_invalid(&agent));
        graph.plan_agent_path(&mut agent, start);
        assert!(!agent.cached_path.is_empty());
        assert!(!graph.path_cache_invalid(&agent), "fresh plan is reusable");

        // Sub-epsilon jitter does NOT trigger a re-plan.
        agent.target += Vec3::new(0.1, 0.0, 0.0);
        assert!(!graph.path_cache_invalid(&agent));

        // A meaningful target move past the epsilon invalidates the cache.
        agent.target += Vec3::new(3.0, 0.0, 0.0);
        assert!(graph.path_cache_invalid(&agent));
    }

    #[test]
    fn rebake_invalidates_cached_path() {
        let mut graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 10.0));
        graph.plan_agent_path(&mut agent, Vec3::new(1.0, 0.0, 1.0));
        assert!(!graph.path_cache_invalid(&agent));

        // A rebake bumps the generation; the agent's path is now stale.
        graph.bake(&Scene::new());
        assert!(graph.path_cache_invalid(&agent));
    }

    #[test]
    fn cached_path_ages_out_after_max_frames() {
        let graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 10.0));
        graph.plan_agent_path(&mut agent, Vec3::new(1.0, 0.0, 1.0));
        assert!(!graph.path_cache_invalid(&agent));

        agent.frames_since_replan = MAX_PATH_AGE_FRAMES;
        assert!(graph.path_cache_invalid(&agent));
    }

    #[test]
    fn agent_reaches_goal_through_repeated_ticks() {
        let mut scene = Scene::new();
        let id = scene.add_entity("agent".to_string());
        {
            let mut e = scene.get_entity_mut(id).expect("entity exists");
            e.transform.position = Vec3::new(1.0, 0.0, 1.0);
            e.nav_agent = Some(test_agent(Vec3::new(10.0, 0.0, 10.0)));
        }
        let graph = open_graph();
        let dt = 1.0 / 60.0;
        for _ in 0..2000 {
            graph.tick_nav_agents(&mut scene, dt);
        }

        let pos = scene
            .get_entity(id)
            .expect("entity exists")
            .transform
            .position;
        let dx = pos.x - 10.0;
        let dz = pos.z - 10.0;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(dist <= 0.6, "agent should arrive near the goal, got {dist}");
    }

    #[test]
    fn tick_results_are_deterministic_across_runs() {
        let run = || {
            let mut scene = Scene::new();
            let id = scene.add_entity("agent".to_string());
            {
                let mut e = scene.get_entity_mut(id).expect("entity exists");
                e.transform.position = Vec3::new(1.0, 0.0, 1.0);
                e.nav_agent = Some(test_agent(Vec3::new(15.0, 0.0, 8.0)));
            }
            let graph = open_graph();
            let dt = 1.0 / 60.0;
            for _ in 0..300 {
                graph.tick_nav_agents(&mut scene, dt);
            }
            let pos = scene
                .get_entity(id)
                .expect("entity exists")
                .transform
                .position;
            pos.to_array()
        };
        assert_eq!(run(), run(), "cached pathing must be deterministic");
    }
}
