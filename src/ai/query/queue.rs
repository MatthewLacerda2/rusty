//! The time-sliced environment query (#2116): Unreal's EQS manager, which runs
//! its queries a slice per tick (`MaxAllowedTestingTime`), with the slice
//! counted in navmesh node expansions instead of a clock so the sim stays a
//! function of its inputs, as the agents' path queue counts its own
//! (`navigation::request`).
//!
//! A request is [`Progress::start`]ed when it is made, so it is generated and
//! measured against the world of that tick; then it waits here for its `path`
//! tests' search. Each tick [`QueryQueue::serve`] spends one budget of
//! expansions on the waiting queries, oldest first (made in the same tick: in
//! the order they were made), resuming the search the last tick left unfinished.
//! A query whose search ends is ranked and picked on that tick, and its answer
//! waits for its ticket to be polled. A query with no `path` test needs no
//! search, and is answered when it is made.
//!
//! The search is stepped on the same heap and costs from tick to tick, so it pops
//! the spans a synchronous one pops, in the same order: an answer is exactly
//! what [`super::run`] gives for the world the query was made in.

use std::collections::VecDeque;

use super::progress::Progress;
use super::rank::pick;
use super::spec::RunMode;
use super::Item;
use crate::core::random::Random;
use crate::navigation::{FloodSearch, NavigationGraph};

/// The node expansions the queue spends each tick by default. Picked on the
/// Dust2 arena (#2116): one query's search toward a 24-item donut 20 m away
/// takes about 7 000, so most answers arrive within a few ticks while the
/// search's share of a tick stays well under a millisecond.
pub const DEFAULT_ITERATIONS_PER_FRAME: u32 = 2_048;

/// A request's handle, never 0.
pub type Ticket = u32;

/// Where a request stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    /// Still waiting for its search.
    Pending,
    /// Answered: the item its mode picked, `None` when nothing survived.
    Done(Option<Item>),
    /// No such request: never made, cancelled, already read, or dropped (its
    /// querier destroyed, a Stop or a scene load since).
    Gone,
}

/// One query waiting for its search.
struct Waiting {
    ticket: Ticket,
    /// The querier: a destroyed one's requests go with it.
    owner: u32,
    progress: Progress,
    mode: RunMode,
    /// Whether the queue's search is this query's, for its current `path` test.
    begun: bool,
}

/// The waiting queries, the search of the one at the head, and the answers not
/// read yet.
pub struct QueryQueue {
    pending: VecDeque<Waiting>,
    /// The head's search, with a pool of its own: a synchronous query meanwhile
    /// never touches its costs.
    search: FloodSearch,
    answered: Vec<(Ticket, u32, Option<Item>)>,
    last_ticket: Ticket,
    iterations: u32,
}

impl Default for QueryQueue {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            search: FloodSearch::default(),
            answered: Vec::new(),
            last_ticket: 0,
            iterations: DEFAULT_ITERATIONS_PER_FRAME,
        }
    }
}

impl QueryQueue {
    /// Queue `progress`, made by querier `owner`, to be picked from by `mode`:
    /// its ticket. One with no search left is ranked and picked now, from `rng`.
    pub fn request(
        &mut self,
        owner: u32,
        progress: Progress,
        mode: RunMode,
        rng: &mut Random,
    ) -> Ticket {
        self.last_ticket = self.last_ticket.wrapping_add(1).max(1);
        let ticket = self.last_ticket;
        if progress.ask().is_none() {
            let item = pick(&progress.ranked(), mode, rng);
            self.answered.push((ticket, owner, item));
        } else {
            self.pending.push_back(Waiting {
                ticket,
                owner,
                progress,
                mode,
                begun: false,
            });
        }
        ticket
    }

    /// Where `ticket` stands. Reading an answer hands it over: the ticket reads
    /// as [`Status::Gone`] after.
    pub fn poll(&mut self, ticket: Ticket) -> Status {
        if let Some(i) = self.answered.iter().position(|a| a.0 == ticket) {
            return Status::Done(self.answered.remove(i).2);
        }
        if self.pending.iter().any(|w| w.ticket == ticket) {
            Status::Pending
        } else {
            Status::Gone
        }
    }

    /// Drop the request `ticket`, waiting or answered: whether there was one.
    pub fn cancel(&mut self, ticket: Ticket) -> bool {
        let (waiting, answered) = (self.pending.len(), self.answered.len());
        self.pending.retain(|w| w.ticket != ticket);
        self.answered.retain(|a| a.0 != ticket);
        waiting + answered != self.pending.len() + self.answered.len()
    }

    /// Spend this tick's budget on the waiting queries, head first, ranking and
    /// picking (from `rng`) each one whose search ends.
    pub fn serve(&mut self, nav: &NavigationGraph, rng: &mut Random) {
        let mut budget = self.iterations;
        while budget > 0 {
            let Some(head) = self.pending.front_mut() else {
                return;
            };
            let Some(ask) = head.progress.ask() else {
                self.finish(rng);
                continue;
            };
            if !head.begun {
                ask.begin(nav, &mut self.search);
                head.begun = true;
            }
            if !nav.step_flood(&mut self.search, &mut budget) {
                return; // out of budget: the search resumes next tick
            }
            let lengths = ask.read(nav, &self.search);
            head.progress.answer(lengths);
            head.begun = false;
            if head.progress.ask().is_none() {
                self.finish(rng);
            }
        }
    }

    /// Rank the head query, pick from it and keep its answer for its ticket.
    fn finish(&mut self, rng: &mut Random) {
        if let Some(w) = self.pending.pop_front() {
            let item = pick(&w.progress.ranked(), w.mode, rng);
            self.answered.push((w.ticket, w.owner, item));
        }
    }

    /// Keep only the requests whose querier `keep` accepts.
    pub fn retain_owners(&mut self, keep: impl Fn(u32) -> bool) {
        self.pending.retain(|w| keep(w.owner));
        self.answered.retain(|a| keep(a.1));
    }

    /// Drop every request, keeping the budget and the ticket count, so a ticket
    /// from before reads as gone: a scene load.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.answered.clear();
    }

    /// How many queries wait for their search.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// The node expansions the queue spends each tick.
    pub fn iterations_per_frame(&self) -> u32 {
        self.iterations
    }

    /// Set [`Self::iterations_per_frame`], at least 1 so the queue always moves.
    pub fn set_iterations_per_frame(&mut self, iterations: u32) {
        self.iterations = iterations.max(1);
    }
}
