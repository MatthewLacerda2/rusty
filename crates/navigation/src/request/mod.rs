//! A request names only the agent: when its search begins it reads where the agent stands, its target and area mask then, so a waiting request plans from where the agent is now.
//! Determinism: the budget is a count of expansions, never a clock; a search resumes on the same heap and node pool, so it finds the path a synchronous search would.

mod carry;
pub(crate) mod islands;
mod serve;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use glam::Vec3;

use super::astar::Search;
use super::pool::NodePool;

/// The node expansions the queue spends each tick by default (Unity's `pathfindingIterationsPerFrame` counts polygons, 100 by default; rusty's nodes are grid spans, many to a polygon).
pub const DEFAULT_ITERATIONS_PER_FRAME: u32 = 2_048;

#[derive(Clone, Copy)]
struct Request {
    ticket: u32,
    agent: u32,
}

struct Active {
    search: Search,
    from: Vec3,
    goal: Vec3,
    target: Vec3,
    mask: u32,
    generation: u64,
}

pub(crate) struct PathQueue {
    pending: VecDeque<Request>,
    active: Option<Active>,
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
    /// Queue a request for `agent` and return its ticket, never 0.
    fn push(&mut self, agent: u32) -> u32 {
        self.last_ticket = self.last_ticket.wrapping_add(1).max(1);
        let ticket = self.last_ticket;
        self.pending.push_back(Request { ticket, agent });
        ticket
    }

    fn holds(&self, ticket: u32) -> bool {
        self.pending.iter().any(|r| r.ticket == ticket)
    }

    #[cfg(test)]
    pub(crate) fn issued(&self) -> u32 {
        self.last_ticket
    }
}

impl super::NavigationGraph {
    pub fn pathfinding_iterations_per_frame(&self) -> u32 {
        self.requests.borrow().iterations
    }

    pub fn set_pathfinding_iterations_per_frame(&self, iterations: u32) {
        self.requests.borrow_mut().iterations = iterations.max(1);
    }

    pub fn reset_path_queue(&self) {
        *self.requests.borrow_mut() = PathQueue::default();
    }

    pub fn pending_path_requests(&self) -> usize {
        self.requests.borrow().pending.len()
    }
}
