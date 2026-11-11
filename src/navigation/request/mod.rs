//! src/navigation/request/ — the agents' path queue (#2087): Unity's time-sliced
//! path requests (`NavMesh.pathfindingIterationsPerFrame`, `NavMeshAgent.pathPending`).
//!
//! An agent whose path went stale does not plan on the spot: it asks the graph's
//! [`PathQueue`] and keeps following its old path (none, for a new agent) until the
//! answer arrives. Each tick the queue spends a fixed count of A\* node expansions
//! on its requests, oldest first (one tick's requests in ECS order), resuming the
//! search the last tick left unfinished. A scene with few agents gets every answer
//! in the tick it asked, as before.
//!
//! A request names only the agent. When its search begins, it reads where the agent
//! stands, its target and its area mask *then*, so a request that waited plans from
//! where the agent is now, to where it wants to go now. The agent holds the request's
//! ticket (`path_request`); a request whose agent no longer holds it (a `ResetPath`,
//! a `Warp`, a removed agent) is dropped when it comes up.
//!
//! Split: this file is the queue and its budget; `serve` runs it each tick and hands
//! the paths to the agents.
//!
//! Determinism: the budget is a count of expansions, never a clock; the order is the
//! queue's; a search resumes on the same heap and node pool, so it finds the path a
//! synchronous search from the same start would have (`NavigationGraph::step_search`).
//! A rebake mid-search restarts it on the new navmesh.

mod serve;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use glam::Vec3;

use super::astar::Search;
use super::pool::NodePool;

/// The node expansions the queue spends each tick by default (Unity's
/// `pathfindingIterationsPerFrame`, there counted in polygons and 100 by default;
/// rusty's nodes are grid spans, many to a polygon). Measured on the Dust2 arena
/// with 32 bots (#2087): see the constant's PR for the figures.
pub const DEFAULT_ITERATIONS_PER_FRAME: u32 = 4_096;

/// One agent waiting for a path.
#[derive(Clone, Copy)]
struct Request {
    ticket: u32,
    agent: u32,
}

/// The search of the request at the queue's head, carried from tick to tick.
struct Active {
    search: Search,
    /// Where the agent's feet were, and the walkable point near its target it heads
    /// for, when the search began.
    from: Vec3,
    goal: Vec3,
    /// The agent's raw target then: its path's `planned_target`.
    target: Vec3,
    mask: u32,
    /// The bake the search runs on; a rebake restarts it.
    generation: u64,
}

/// The graph's queue of path requests and the search in progress.
pub(crate) struct PathQueue {
    pending: VecDeque<Request>,
    active: Option<Active>,
    /// The active search's costs and parents, kept apart from the graph's shared
    /// pool so a script's synchronous query never touches them.
    pool: NodePool,
    last_ticket: u32,
    iterations: u32,
}

impl Default for PathQueue {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            active: None,
            pool: NodePool::default(),
            last_ticket: 0,
            iterations: DEFAULT_ITERATIONS_PER_FRAME,
        }
    }
}

impl PathQueue {
    /// Queue a request for `agent` and return its ticket, never 0 (0 is "none").
    fn push(&mut self, agent: u32) -> u32 {
        self.last_ticket = self.last_ticket.wrapping_add(1).max(1);
        let ticket = self.last_ticket;
        self.pending.push_back(Request { ticket, agent });
        ticket
    }

    /// Whether the request with `ticket` is still waiting.
    fn holds(&self, ticket: u32) -> bool {
        self.pending.iter().any(|r| r.ticket == ticket)
    }

    /// How many requests have been queued since the graph was made (a test probe).
    #[cfg(test)]
    pub(crate) fn issued(&self) -> u32 {
        self.last_ticket
    }
}

impl super::NavigationGraph {
    /// The node expansions the agents' path queue spends each tick (Unity's
    /// `NavMesh.pathfindingIterationsPerFrame`).
    pub fn pathfinding_iterations_per_frame(&self) -> u32 {
        self.requests.borrow().iterations
    }

    /// Set [`Self::pathfinding_iterations_per_frame`], at least 1 so the queue
    /// always moves. It is the queue's, and survives a rebake.
    pub fn set_pathfinding_iterations_per_frame(&self, iterations: u32) {
        self.requests.borrow_mut().iterations = iterations.max(1);
    }

    /// How many agents are waiting for a path, the one being searched included.
    pub fn pending_path_requests(&self) -> usize {
        self.requests.borrow().pending.len()
    }
}
