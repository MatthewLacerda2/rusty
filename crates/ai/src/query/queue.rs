use std::collections::VecDeque;

use super::progress::Progress;
use super::rank::pick;
use super::spec::RunMode;
use super::Item;
use rusty_core::random::Random;
use rusty_navigation::{FloodSearch, NavigationGraph};

/// Node expansions a tick by default, spent on the waiting queries oldest first.
pub const DEFAULT_ITERATIONS_PER_FRAME: u32 = 2_048;

/// A request's handle, never 0.
pub type Ticket = u32;

/// Where a request stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    /// Still waiting for its search.
    Pending,
    Done(Option<Item>),
    Gone,
}

struct Waiting {
    ticket: Ticket,
    /// The querier: a destroyed one's requests go with it.
    owner: u32,
    progress: Progress,
    mode: RunMode,
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

    pub fn cancel(&mut self, ticket: Ticket) -> bool {
        let (waiting, answered) = (self.pending.len(), self.answered.len());
        self.pending.retain(|w| w.ticket != ticket);
        self.answered.retain(|a| a.0 != ticket);
        waiting + answered != self.pending.len() + self.answered.len()
    }

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

    fn finish(&mut self, rng: &mut Random) {
        if let Some(w) = self.pending.pop_front() {
            let item = pick(&w.progress.ranked(), w.mode, rng);
            self.answered.push((w.ticket, w.owner, item));
        }
    }

    pub fn retain_owners(&mut self, keep: impl Fn(u32) -> bool) {
        self.pending.retain(|w| keep(w.owner));
        self.answered.retain(|a| keep(a.1));
    }

    /// Keeps `last_ticket`, so a ticket from before reads as gone, never as a
    /// later request's.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.answered.clear();
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    pub fn iterations_per_frame(&self) -> u32 {
        self.iterations
    }

    pub fn set_iterations_per_frame(&mut self, iterations: u32) {
        self.iterations = iterations.max(1);
    }
}

impl QueryQueue {
    pub fn shift_origin(&mut self, by: glam::Vec3) {
        for w in &mut self.pending {
            w.progress.shift_origin(by);
        }
        for (_, _, item) in &mut self.answered {
            if let Some(item) = item.as_mut() {
                item.position += by;
            }
        }
    }
}
