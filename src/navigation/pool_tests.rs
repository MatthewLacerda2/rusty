use super::*;
use crate::navigation::test_support::{add_box, add_floor, bake_pinned};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

fn span(index: u32) -> SpanRef {
    let (gx, gz) = (0, 0);
    SpanRef { gx, gz, index }
}

/// A new search reads every slot as unreached, whatever the last one wrote.
#[test]
fn a_new_generation_hides_the_last_search() {
    let scratch = SearchScratch::default();
    scratch.with(4, |p| p.set(2, 5.0, Some(span(1))));
    scratch.with(4, |p| {
        assert_eq!(p.g(2), f32::INFINITY);
        assert_eq!(p.came_from(2), None);
        p.set(3, 1.0, None);
        assert_eq!(p.g(3), 1.0);
    });
}

/// A search nested inside another gets a pool of its own, not the outer one's slots.
#[test]
fn a_nested_search_runs_on_its_own_pool() {
    let scratch = SearchScratch::default();
    scratch.with(4, |outer| {
        outer.set(1, 2.0, None);
        scratch.with(4, |inner| {
            assert_eq!(inner.g(1), f32::INFINITY);
            inner.set(1, 9.0, None);
        });
        assert_eq!(outer.g(1), 2.0, "the inner search left the outer one alone");
    });
}

/// A search that panics leaves the scratch borrowable, its slots stale.
#[test]
fn a_panicked_search_leaves_the_pool_usable() {
    let scratch = SearchScratch::default();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scratch.with(4, |p| {
            p.set(0, 3.0, None);
            panic!("mid-search");
        })
    }));
    assert!(caught.is_err());
    scratch.with(4, |p| assert_eq!(p.g(0), f32::INFINITY));
}

/// The stamp wrapping round clears the slots, so a 2³²-old slot can't read as current.
#[test]
fn a_wrapped_generation_clears_the_stamps() {
    let mut pool = NodePool::default();
    pool.begin(2);
    pool.set(0, 1.0, None);
    pool.generation = u32::MAX;
    pool.set(1, 1.0, None);
    pool.begin(2); // wraps to 0, restarts at 1, the stamp slot 0 carries
    assert_eq!((pool.g(0), pool.g(1)), (f32::INFINITY, f32::INFINITY));
}

/// A level with walls, a sealed pillar (a `Partial` target) and, with `deck`, a
/// slab overhead that stacks a second span on its cells, on the pinned 1 m grid.
fn scene(deck: bool) -> Scene {
    let mut scene = Scene::default();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(4.0, 0.0, 0.0),
        Vec3::new(5.0, 2.5, 7.0),
    );
    add_box(
        &mut scene,
        Vec3::new(7.0, 0.0, 6.0),
        Vec3::new(8.0, 4.0, 7.0),
    );
    if deck {
        add_box(
            &mut scene,
            Vec3::new(1.0, 3.0, 7.0),
            Vec3::new(3.0, 3.1, 10.0),
        );
    }
    scene
}

fn level(deck: bool) -> NavigationGraph {
    bake_pinned(&mut scene(deck))
}

const QUERIES: [((f32, f32), (f32, f32)); 6] = [
    ((0.5, 0.5), (9.5, 0.5)),
    ((9.5, 9.5), (0.5, 0.5)),
    ((2.0, 9.0), (8.0, 2.0)),
    ((0.5, 0.5), (7.5, 6.5)),
    ((3.0, 3.0), (3.0, 3.0)),
    ((6.0, 1.0), (6.5, 1.5)),
];

/// The queries `order` picks, each as its index and its status and corners, bit
/// for bit, sorted back into `QUERIES` order.
fn run(g: &NavigationGraph, order: impl Iterator<Item = usize>) -> Vec<(usize, String)> {
    let mut out: Vec<_> = order
        .map(|i| {
            let ((ax, az), (bx, bz)) = QUERIES[i];
            let p = g.calculate_path(Vec3::new(ax, 0.0, az), Vec3::new(bx, 0.0, bz));
            let bits: Vec<_> = p
                .corners
                .iter()
                .map(|c| c.to_array().map(f32::to_bits))
                .collect();
            (i, format!("{:?} {bits:?}", p.status))
        })
        .collect();
    out.sort_by_key(|(i, _)| *i);
    out
}

/// Reusing the pool never changes a path (#2083): the same queries in two orders,
/// on a pool that has served other searches, and on a pool grown by a rebake give
/// the same bytes as a fresh graph per query.
#[test]
fn reused_pool_gives_the_same_paths_in_any_order() {
    let n = QUERIES.len();
    let fresh: Vec<_> = (0..n)
        .flat_map(|i| run(&level(true), std::iter::once(i)))
        .collect();
    let partial = fresh.iter().any(|(_, s)| s.starts_with("Partial"));
    assert!(partial, "the pillar's query is partial: {fresh:?}");
    let g = level(true);
    assert_eq!(run(&g, 0..n), fresh, "forward order");
    assert_eq!(run(&g, (0..n).rev()), fresh, "reverse order");

    // Searches on the smaller mesh first, then a rebake that stacks the deck's spans.
    let mut grown = level(false);
    let before = grown.spans.len();
    run(&grown, 0..n);
    let mut deck = scene(true);
    bake_pinned(&mut deck); // pins the scene's bake settings to the grid's
    grown.bake(&deck);
    assert!(grown.spans.len() > before, "the rebake grew the mesh");
    assert_eq!(run(&grown, (0..n).rev()), fresh, "after a growing rebake");
}
