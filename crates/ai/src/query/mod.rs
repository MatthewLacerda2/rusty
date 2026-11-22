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

pub fn run(world: &QueryWorld, querier: Anchor, spec: &QuerySpec) -> Vec<Item> {
    let mut progress = Progress::start(world, querier, spec.clone());
    while let Some(ask) = progress.ask() {
        let lengths = ask.lengths(world.nav);
        progress.answer(lengths);
    }
    progress.ranked()
}
