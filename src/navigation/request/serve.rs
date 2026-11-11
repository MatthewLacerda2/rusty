//! The queue's two steps each tick (#2087): the agents ask for the paths they need
//! ([`NavigationGraph::ask_paths`]), then the queue spends its budget answering
//! them ([`NavigationGraph::serve_paths`]).

use super::super::agents::TARGET_SAMPLE_DISTANCE;
use super::super::{NavPath, NavigationGraph};
use super::{Active, PathQueue, Request};
use crate::scene::{NavMeshAgentComponent, Scene};

/// How the request at the head starts: answered at once, or a search to run.
enum Begun {
    Answered(NavPath, glam::Vec3),
    Searching(Active),
}

impl NavigationGraph {
    /// Every active agent that needs a new path and is not already waiting for one
    /// queues a request, in ECS order. A wait the queue lost (the graph was
    /// replaced) is asked again.
    pub(in crate::navigation) fn ask_paths(&self, scene: &mut Scene) {
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

    /// Whether `agent`, standing at `position`, is waiting for a path or will ask
    /// for one on its next tick (Unity's `pathPending`).
    pub fn path_pending(&self, agent: &NavMeshAgentComponent, position: glam::Vec3) -> bool {
        agent.path_request != 0 || self.needs_path(agent, position)
    }

    /// Spend this tick's budget on the queue, head first, handing each finished path
    /// to its agent. A request whose agent gave up its ticket is dropped unsearched.
    pub(in crate::navigation) fn serve_paths(&self, scene: &mut Scene) {
        let mut queue = self.requests.borrow_mut();
        let mut budget = queue.iterations;
        while let Some(&Request { ticket, agent: id }) = queue.pending.front() {
            let wanted = (scene.world.nav_agent(id)).is_some_and(|a| a.path_request == ticket);
            if !wanted {
                queue.pending.pop_front();
                queue.active = None;
                continue;
            }
            if budget == 0 {
                break;
            }
            let Some((path, target)) = self.advance(&mut queue, scene, id, &mut budget) else {
                break; // out of budget: the search resumes next tick
            };
            queue.pending.pop_front();
            if let Some(mut agent) = scene.world.nav_agent_mut(id) {
                self.install_path(&mut agent, path, target);
            }
        }
    }

    /// Run the head request (agent `id`) on `budget`: its path and the target it
    /// was planned for once found, `None` when the budget ran out first.
    fn advance(
        &self,
        queue: &mut PathQueue,
        scene: &Scene,
        id: u32,
        budget: &mut u32,
    ) -> Option<(NavPath, glam::Vec3)> {
        let PathQueue { active, pool, .. } = queue;
        if active
            .as_ref()
            .is_some_and(|a| a.generation != self.bake_generation)
        {
            *active = None; // a rebake since it began: search the new navmesh
        }
        if active.is_none() {
            match self.begin_request(pool, scene, id) {
                Begun::Answered(path, target) => return Some((path, target)),
                Begun::Searching(a) => *active = Some(a),
            }
        }
        let a = active.as_mut()?;
        let (spans, complete) = self.step_search(&mut a.search, pool, budget)?;
        let a = active.take()?;
        let path = self.route_path((a.from, a.goal), &spans, complete, a.mask);
        Some((path, a.target))
    }

    /// Begin agent `id`'s search from where it stands now toward the walkable point
    /// nearest its target now (within [`TARGET_SAMPLE_DISTANCE`], #666): exactly the
    /// ends a synchronous `calculate_path_masked` would snap. With no walkable point
    /// near either end the answer is no path, at once.
    fn begin_request(&self, pool: &mut super::NodePool, scene: &Scene, id: u32) -> Begun {
        let position = scene.world.world_position(id);
        let Some((from, target, mask)) = scene
            .world
            .nav_agent(id)
            .map(|a| (a.feet(position), a.target, a.area_mask))
        else {
            return Begun::Answered(NavPath::invalid(), position);
        };
        let goal = self.sample_position(target, TARGET_SAMPLE_DISTANCE);
        let ends = goal.and_then(|g| Some((self.snap(from)?, self.snap(g)?, g)));
        let Some((start, end, goal)) = ends else {
            return Begun::Answered(NavPath::invalid(), target);
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
}
