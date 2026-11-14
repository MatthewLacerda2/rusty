//! crates/ai/src/query/ — the environment query (#1892): Unreal's EQS, so an agent can
//! ask "where should I stand?".
//!
//! A query runs in three steps:
//!
//! 1. a **generator** (`generate`) makes candidate items around a context and
//!    projects each onto the navmesh, dropping the ones with no floor near;
//! 2. **tests** measure every item (`measure`, `sight`, `walk`): each may filter
//!    items out and/or score them. As in Unreal, the cheap tests run first
//!    whatever order the spec writes them in ([`Measure::cost`]: maths, then rays,
//!    then the navmesh), so the dear ones only see the survivors;
//! 3. **scoring** (`rank`) normalizes each scoring test's raw values to 0..1 over
//!    the survivors, shapes them by its curve, weighs them and sums; the items are
//!    ranked best first, a tie to the earliest generated, and [`pick`] chooses
//!    one by [`RunMode`].
//!
//! Synchronous (decision 1): [`run`] answers in the call that runs it. It is a
//! pure function of the scene, the navmesh, the physics world, the active camera
//! and the spec; only [`pick`]'s random modes draw from the seeded
//! [`Random`](rusty_core::random::Random), once per query that found anything.
//!
//! Time-sliced (#2116): [`QueryQueue`] runs the same query in stages, as
//! Unreal's EQS manager does: generation and every test but `path` when it is
//! requested ([`Progress`]), then the `path` tests' search on a per-tick budget of
//! node expansions shared by every query waiting, oldest first; the ranking and
//! the pick happen on the tick the search finishes. Its answers are exactly
//! [`run`]'s for the same world as it was when the query was requested.

mod generate;
mod measure;
mod progress;
pub mod queue;
mod rank;
mod sight;
pub mod spec;
mod walk;

#[cfg(test)]
mod carry_tests;
#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod generate_tests;
// Test hook: the bench level the root's `dev::bench` times.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
#[cfg(all(test, feature = "dev"))]
mod query_cost_tests;
#[cfg(test)]
mod queue_tests;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod spread_tests;
// Test hook: the arena this module's tests and `rusty-script`'s
// `api::environment_query` tests build on. Ungated: a `dev` dev-dependency would
// split the crate's features between its build and its tests.
#[doc(hidden)]
pub mod test_world;

use glam::{Vec2, Vec3};

pub use progress::Progress;
pub use queue::{QueryQueue, Ticket, DEFAULT_ITERATIONS_PER_FRAME};
pub use rank::pick;
pub use spec::{
    Anchor, Curve, Filter, Generator, Measure, QuerySpec, RunMode, Score, Test, DEFAULT_PROJECT,
    MAX_ITEMS,
};
pub use walk::FloodAsk;

use rusty_navigation::NavigationGraph;
use rusty_physics::PhysicsWorld;
use rusty_scene::{Camera, Scene};

/// What a query reads: the engine state it measures items against.
pub struct QueryWorld<'a> {
    pub scene: &'a Scene,
    pub nav: &'a NavigationGraph,
    /// The live physics world; `None` before Play builds one, when no ray is
    /// blocked (as `Physics.Raycast` misses).
    pub physics: Option<&'a PhysicsWorld>,
    /// The active view (`scene::game_camera_from_scene`), what `view` tests read.
    pub view: &'a Camera,
    /// The screen the view projects onto, pixels: its aspect shapes the frustum.
    pub screen: Vec2,
}

/// One ranked item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub position: Vec3,
    pub score: f32,
}

/// Run `spec` for `querier`: every surviving item, best first. Empty when
/// nothing survives (never an error: the spec was checked when it was built).
/// The `path` tests' search runs to its end in the call; [`QueryQueue`] runs
/// the same query with that search sliced across ticks.
pub fn run(world: &QueryWorld, querier: Anchor, spec: &QuerySpec) -> Vec<Item> {
    let mut progress = Progress::start(world, querier, spec.clone());
    while let Some(ask) = progress.ask() {
        let lengths = ask.lengths(world.nav);
        progress.answer(lengths);
    }
    progress.ranked()
}
