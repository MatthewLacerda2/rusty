use glam::Vec3;

use super::super::agents::{TARGET_SAMPLE_DISTANCE, WAYPOINT_REACHED_DISTANCE};
use super::super::{NavPath, NavigationGraph};
use super::{Active, PathQueue, Request};
use rusty_scene::{NavMeshAgentComponent, Scene};

type Ends = (Vec3, Vec3, u32);

type Answer = (NavPath, Vec3, Vec3, bool);

enum Begun {
    Answered(Answer),
    Searching(Active),
}

impl NavigationGraph {
    pub(crate) fn ask_paths(&self, scene: &mut Scene) {
        let mut queue = self.requests.borrow_mut();
        for id in scene.world.ids_with_nav_agent() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let position = scene.world.world_position(id);
            let Some(mut agent) = scene.world.nav_agent_mut(id) else {
                continue;
            };
            if agent.path_request != 0 && queue.holds(agent.path_request) {
                continue;
            }
            agent.path_request = 0;
            let feet = agent.feet(position);
            self.keep_path_if_untouched(&mut agent, feet);
            if self.needs_path(&agent, position) {
                agent.path_request = queue.push(id);
            }
        }
    }

    pub fn path_pending(&self, agent: &NavMeshAgentComponent, position: Vec3) -> bool {
        let current = self.path_untouched(agent, agent.feet(position));
        agent.path_request != 0 || self.needs_path_on(agent, position, current)
    }

    pub(crate) fn serve_paths(&self, scene: &mut Scene) {
        let mut queue = self.requests.borrow_mut();
        let mut budget = queue.iterations;
        while let Some(&Request { ticket, agent: id }) = queue.pending.front() {
            let Some(ends) = wanted(scene, id, ticket) else {
                queue.pending.pop_front();
                queue.active = None;
                continue;
            };
            if budget == 0 {
                break;
            }
            let Some((path, target, from, anchored)) = self.advance(&mut queue, ends, &mut budget)
            else {
                break; // out of budget: the search resumes next tick
            };
            queue.pending.pop_front();
            let feet_now = scene.world.world_position(id);
            if let Some(mut agent) = scene.world.nav_agent_mut(id) {
                self.install_path(&mut agent, path, target);
                agent.replan_from = anchored.then_some(from);
                let feet = agent.feet(feet_now);
                self.skip_passed(&mut agent, from, feet);
            }
        }
    }

    fn advance(&self, queue: &mut PathQueue, ends: Ends, budget: &mut u32) -> Option<Answer> {
        let PathQueue { active, pool, .. } = queue;
        if active
            .as_ref()
            .is_some_and(|a| a.generation != self.bake_generation)
        {
            *active = None; // a rebake `carry` did not carry it over: search afresh
        }
        if active.is_none() {
            match self.begin_request(pool, ends) {
                Begun::Answered(answer) => return Some(answer),
                Begun::Searching(a) => *active = Some(a),
            }
        }
        let a = active.as_mut()?;
        let (spans, complete) = self.step_search(&mut a.search, pool, budget)?;
        let a = active.take()?;
        let path = self.route_path((a.from, a.goal), &spans, complete, a.mask);
        let capped = !complete && a.search.capped();
        Some((path, a.target, a.from, capped))
    }

    /// Begin a search from the agent's feet toward the walkable point nearest its target (within [`TARGET_SAMPLE_DISTANCE`]): exactly the ends a synchronous `calculate_path_masked` would snap.
    fn begin_request(&self, pool: &mut super::NodePool, (from, target, mask): Ends) -> Begun {
        let goal = self.sample_position(target, TARGET_SAMPLE_DISTANCE);
        let start = self.snap(from);
        let ends = goal.and_then(|g| Some((start?, self.snap(g)?, g)));
        let Some((start, end, goal)) = ends else {
            let answer = (NavPath::invalid(), target, from, start.is_none());
            return Begun::Answered(answer);
        };
        pool.begin(self.spans.len());
        Begun::Searching(Active {
            search: self.begin_search(start, end, mask, pool),
            from,
            goal,
            target,
            mask,
            generation: self.bake_generation,
        })
    }

    /// Skip the waypoints at the head of a just-delivered path that the agent walked past while it waited, so a chaser does not turn back to touch a corner.
    fn skip_passed(&self, agent: &mut NavMeshAgentComponent, from: Vec3, feet: Vec3) {
        let walked = from.distance(feet) + WAYPOINT_REACHED_DISTANCE;
        let (mut along, mut prev) = (0.0, from);
        while let [next, after, ..] = agent.cached_path[agent.path_cursor..] {
            along += prev.distance(next);
            prev = next;
            let link = agent
                .path_links
                .iter()
                .any(|&(w, _)| w <= agent.path_cursor + 1);
            let seen = self.raycast_masked(feet, after, agent.area_mask);
            if along > walked || link || seen.is_none_or(|h| h.hit) {
                break;
            }
            agent.path_cursor += 1;
        }
    }
}

fn wanted(scene: &mut Scene, id: u32, ticket: u32) -> Option<Ends> {
    let live = scene.world.is_active_in_hierarchy(id);
    let position = scene.world.world_position(id);
    let mut agent = scene.world.nav_agent_mut(id)?;
    if agent.path_request != ticket {
        return None;
    }
    if !(live && agent.active && agent.off_mesh_link.is_none()) {
        agent.path_request = 0;
        return None;
    }
    Some((agent.feet(position), agent.target, agent.area_mask))
}
