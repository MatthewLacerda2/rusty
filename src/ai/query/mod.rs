//! src/ai/query/ — the environment query (#1892): Unreal's EQS, so an agent can
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
//! Synchronous (decision 1): a query answers in the call that runs it. It is a
//! pure function of the scene, the navmesh, the physics world, the active camera
//! and the spec; only [`pick`]'s random modes draw from the seeded
//! [`Random`](crate::core::random::Random), once per query that found anything.

// Panic-free sim (#195): bare `.unwrap()` is denied here.
#![deny(clippy::unwrap_used)]

mod generate;
mod measure;
mod rank;
mod sight;
pub mod spec;
mod walk;

#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod generate_tests;
#[cfg(all(test, feature = "dev"))]
mod query_cost_tests;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod spread_tests;
#[cfg(test)]
pub(crate) mod test_world;

use glam::{Vec2, Vec3};

pub use rank::pick;
pub use spec::{
    Anchor, Curve, Filter, Generator, Measure, QuerySpec, RunMode, Score, Test, DEFAULT_PROJECT,
    MAX_ITEMS,
};

use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::{Camera, Scene};

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
pub fn run(world: &QueryWorld, querier: Anchor, spec: &QuerySpec) -> Vec<Item> {
    let items = generate::items(world.nav, &spec.generator, spec.project);
    let mut alive: Vec<usize> = (0..items.len()).collect();
    let mut raws: Vec<Vec<f32>> = vec![Vec::new(); spec.tests.len()];
    // Cheap first, written order within a cost (a stable sort).
    let mut order: Vec<usize> = (0..spec.tests.len()).collect();
    order.sort_by_key(|&t| spec.tests[t].measure.cost());
    for t in order {
        let test = &spec.tests[t];
        let points: Vec<Vec3> = alive.iter().map(|&i| items[i]).collect();
        let values = measure::raw(world, querier, &test.measure, &points);
        let mut row = vec![f32::NAN; items.len()];
        let mut kept = Vec::with_capacity(alive.len());
        for (&i, value) in alive.iter().zip(values) {
            // `None` fails the test outright (an unreachable item's path length).
            let Some(v) = value else { continue };
            if test.filter.is_none_or(|f| f.keeps(v)) {
                row[i] = v;
                kept.push(i);
            }
        }
        raws[t] = row;
        alive = kept;
    }
    rank::rank(spec, &items, &alive, &raws)
}
