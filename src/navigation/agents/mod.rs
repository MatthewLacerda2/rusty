#[cfg(test)]
mod bounds_tests;
mod changed;
mod link;
pub use link::complete_off_mesh_link;
mod root_motion;
pub mod state;
#[cfg(test)]
mod state_tests;
mod tick;
mod turn;

use super::{NavPath, NavigationGraph};
use crate::scene::NavMeshAgentComponent;
use glam::Vec3;
use state::is_at_target;

/// Squared world-distance the target may drift before a cached agent path is
/// considered stale and re-planned. Keeps tiny jitter from re-running A*.
const REPLAN_TARGET_EPSILON_SQ: f32 = 0.25;
/// How far (world units) from its target the agent looks for the walkable point to
/// head for, when the target itself is off the navmesh (#666). Past it the target is
/// unreachable and the agent plans no path.
pub const TARGET_SAMPLE_DISTANCE: f32 = 8.0;
/// How close (world units, XZ plane) the agent must get to a waypoint before
/// the cursor advances to the next one.
pub(super) const WAYPOINT_REACHED_DISTANCE: f32 = 0.5;

impl NavigationGraph {
    /// Whether `agent`'s cached path must be discarded and re-planned. Re-plan
    /// when none was planned, when a rebake since planning touched it (#456), when the
    /// target drifted past the epsilon, or when the next waypoint is no longer
    /// walkable: on a change, never on a timer (Unity's `autoRepath`, #2087). A path
    /// the agent has walked to the end of stays valid: re-planning it every frame
    /// would change nothing (a partial path ends where the target can't be reached)
    /// and cost a full search each time. So does no path, once planned for this
    /// target on this bake (Unity's `PathInvalid`): asking again would answer the same.
    /// The exception, an answer that held only where it was planned, is
    /// [`outgrown`]'s, which needs where the agent stands.
    #[cfg(test)]
    fn path_cache_invalid(&self, agent: &NavMeshAgentComponent) -> bool {
        self.path_stale(agent, agent.path_generation == self.bake_generation)
    }

    /// [`Self::path_cache_invalid`], `current` saying whether the bake the path was
    /// planned on still holds for it: its own, or a rebake that left it untouched.
    fn path_stale(&self, agent: &NavMeshAgentComponent, current: bool) -> bool {
        if !agent.path_planned || !current {
            return true;
        }
        if (agent.target - agent.planned_target).length_squared() > REPLAN_TARGET_EPSILON_SQ {
            return true;
        }
        // A path walked to its last corner stays valid (see above). Every other corner
        // is a span floor: one gone means the mesh changed.
        if agent.path_cursor + 1 >= agent.cached_path.len() {
            return false;
        }
        self.span_under(agent.cached_path[agent.path_cursor])
            .is_none()
    }

    /// Whether `agent`, at `position`, needs a new path: it is active, not on an
    /// off-mesh link, has somewhere to go (no path and already at its target is
    /// nothing to plan: a `ResetPath` agent plans nothing until it gets a new
    /// target), and its cached path is stale.
    pub(super) fn needs_path(&self, agent: &NavMeshAgentComponent, position: Vec3) -> bool {
        let current = agent.path_generation == self.bake_generation;
        self.needs_path_on(agent, position, current)
    }

    /// [`Self::needs_path`], `current` as [`Self::path_stale`] takes it.
    pub(super) fn needs_path_on(
        &self,
        agent: &NavMeshAgentComponent,
        position: Vec3,
        current: bool,
    ) -> bool {
        agent.active
            && agent.off_mesh_link.is_none()
            && !(agent.cached_path.is_empty() && is_at_target(agent, position))
            && (self.path_stale(agent, current) || outgrown(agent, position))
    }

    /// Store `path`, planned for `target`, on the agent: its corners, links and
    /// status, the cursor and bookkeeping reset, its request answered. The start
    /// corner is dropped (the agent was standing there when the search began).
    pub(super) fn install_path(
        &self,
        agent: &mut NavMeshAgentComponent,
        path: NavPath,
        target: Vec3,
    ) {
        agent.cached_path = path.corners.into_iter().skip(1).collect();
        // Waypoints drop the start corner, so corner `i` is waypoint `i - 1`.
        agent.path_links = path.links.iter().map(|&(i, l)| (i - 1, l)).collect();
        agent.path_cursor = 0;
        agent.path_status = path.status;
        agent.planned_target = target;
        agent.path_generation = self.bake_generation;
        agent.path_request = 0;
        agent.path_planned = true;
        agent.replan_from = None;
    }

    /// Plan the agent's path at once, the way the path queue (`request`) does over
    /// several ticks: from `feet` to the walkable point nearest the target (within
    /// [`TARGET_SAMPLE_DISTANCE`], #666), so the path ends on the navmesh, never on
    /// the raw target; no such point plans no path.
    #[cfg(test)]
    fn plan_agent_path(&self, agent: &mut NavMeshAgentComponent, feet: Vec3) {
        let path = match self.sample_position(agent.target, TARGET_SAMPLE_DISTANCE) {
            Some(goal) => self.calculate_path_masked(feet, goal, agent.area_mask),
            None => NavPath::invalid(),
        };
        self.install_path(agent, path, agent.target);
    }

    /// Returns the world position the agent should steer toward this frame,
    /// using the cached path (the path queue re-plans it, `request`) and advancing the
    /// cursor past any waypoints already reached. Waypoints carry the baked surface
    /// height (#130) so the goal follows ramps/stairs in `y`; the reached test stays
    /// on the XZ plane so a height delta never strands the cursor on a waypoint.
    fn cached_next_step(&self, agent: &mut NavMeshAgentComponent, current_pos: Vec3) -> Vec3 {
        while agent.path_cursor < agent.cached_path.len() {
            let wp = agent.cached_path[agent.path_cursor];
            let dx = wp.x - current_pos.x;
            let dz = wp.z - current_pos.z;
            if (dx * dx + dz * dz).sqrt() > WAYPOINT_REACHED_DISTANCE {
                break;
            }
            agent.path_cursor += 1;
            if link::enter_link(agent) {
                break;
            }
        }
        match agent.cached_path.get(agent.path_cursor) {
            Some(wp) => *wp,
            None => agent.cached_path.last().copied().unwrap_or(current_pos),
        }
    }
}

/// Whether `agent`, at `position`, is done with an answer that held only where it
/// was planned (`replan_from`: a capped search, a start off the navmesh) and now
/// stands past the epsilon from there (#2087, Detour's crowd re-asking at the end of
/// a partial path). An agent still at that spot does not ask again: it would get the
/// same answer, for a capped search's worth of expansions.
fn outgrown(agent: &NavMeshAgentComponent, position: Vec3) -> bool {
    let Some(from) = agent.replan_from else {
        return false;
    };
    let done = agent.path_cursor >= agent.cached_path.len() || is_at_target(agent, position);
    let feet = agent.feet(position);
    done && feet.distance_squared(from) > REPLAN_TARGET_EPSILON_SQ
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    fn open_graph() -> NavigationGraph {
        // 21x21 flat grid, never baked: one open span per cell.
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
    fn off_mesh_target_is_projected_onto_the_navmesh() {
        let mut graph = open_graph();
        let hole = graph.index(10, 20); // the target's column holds no span
        graph.spans.remove(hole);
        for start in &mut graph.cell_start[hole + 1..] {
            *start -= 1;
        }
        let mut agent = test_agent(Vec3::new(10.0, 3.0, 20.0));
        graph.plan_agent_path(&mut agent, Vec3::new(5.0, 0.0, 15.0));
        let end = *agent.cached_path.last().expect("a path");
        assert!(
            graph.span_under(end).is_some(),
            "ends on the navmesh: {end}"
        );
        assert_eq!(end.y, 0.0, "on the floor, not at the target's height");
        assert!(
            end.distance(Vec3::new(10.0, 0.0, 20.0)) < 1.0,
            "nearest point: {end}"
        );
        agent.path_cursor = agent.cached_path.len() - 1;
        assert!(
            !graph.path_cache_invalid(&agent),
            "a walked path stays valid"
        );
    }

    #[test]
    fn target_with_no_navmesh_nearby_plans_no_path() {
        let graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 20.0 + 2.0 * TARGET_SAMPLE_DISTANCE));
        graph.plan_agent_path(&mut agent, Vec3::new(5.0, 0.0, 15.0));
        assert!(agent.cached_path.is_empty());
        assert_eq!(agent.path_status, crate::components::NavPathStatus::Invalid);
    }

    #[test]
    fn agent_reaches_goal_through_repeated_ticks() {
        let mut scene = Scene::new();
        let id = scene.add_entity("agent".to_string());
        scene.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 0.0, 1.0);
        scene
            .world
            .set_nav_agent(id, Some(test_agent(Vec3::new(10.0, 0.0, 10.0))));
        let graph = open_graph();
        let dt = 1.0 / 60.0;
        for _ in 0..2000 {
            graph.tick_nav_agents(&mut scene, dt);
        }

        let pos = scene.world.transform(id).expect("entity exists").position;
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
            scene.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 0.0, 1.0);
            scene
                .world
                .set_nav_agent(id, Some(test_agent(Vec3::new(15.0, 0.0, 8.0))));
            let graph = open_graph();
            let dt = 1.0 / 60.0;
            for _ in 0..300 {
                graph.tick_nav_agents(&mut scene, dt);
            }
            let pos = scene.world.transform(id).expect("entity exists").position;
            pos.to_array()
        };
        assert_eq!(run(), run(), "cached pathing must be deterministic");
    }
}
