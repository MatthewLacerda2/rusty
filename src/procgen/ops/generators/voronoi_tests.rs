//! Voronoi metrics (#1492): each metric's distance on a regular grid, and the search
//! reach checked against every cell point of the tile.

use super::*;
use crate::procgen::recipe::VoronoiMetric::*;
use crate::procgen::recipe::VoronoiOutput::*;

const METRICS: [VoronoiMetric; 3] = [Euclidean, Manhattan, Chebyshev];

#[test]
fn each_metric_gives_its_distance_on_a_regular_grid() {
    // 4 cells, points at the centres; the sample sits (0.2, 0.1) into cell (1, 2),
    // so the nearest centre is (0.3, 0.4) away and the second (0.3, −0.6).
    let (u, v) = ((1.0 + 0.2) / 4.0, (2.0 + 0.1) / 4.0);
    let expected = [
        (Euclidean, 0.5, 0.45f32.sqrt()),
        (Manhattan, 0.7, 0.9),
        (Chebyshev, 0.4, 0.6),
    ];
    for (metric, f1, f2) in expected {
        let at = |output| sampler(4.0, output, metric, Field::Const(0.0), 9)(u, v)[0];
        assert!(
            (at(Distance) - f1).abs() < 1e-5,
            "{metric:?} F1 {}",
            at(Distance)
        );
        assert!((at(F2) - f2).abs() < 1e-5, "{metric:?} F2 {}", at(F2));
        assert!((at(Edges) - (f2 - f1)).abs() < 1e-5, "{metric:?} edges");
    }
}

/// The two smallest distances from `(px, py)` (in cells) to every point of the
/// `cells`² tile and its eight wrapped copies, and the (wrapped) cell of the
/// nearest: what the sampler must find.
fn brute_force(
    cells: i64,
    metric: VoronoiMetric,
    seed: u64,
    px: f32,
    py: f32,
) -> (f32, f32, (i64, i64)) {
    let mut d: Vec<(f32, (i64, i64))> = Vec::new();
    for cy in -cells..2 * cells {
        for cx in -cells..2 * cells {
            let (wx, wy) = (cx.rem_euclid(cells), cy.rem_euclid(cells));
            let at = |ch| hash::unit2(wx, wy, ch, seed);
            let dist = measure(metric, cx as f32 + at(1) - px, cy as f32 + at(2) - py);
            d.push((dist, (wx, wy)));
        }
    }
    d.sort_by(|a, b| a.0.total_cmp(&b.0));
    (d[0].0, d[1].0, d[0].1)
}

/// Samples where one ring finds the wrong point, so a reach cut to 1 fails them:
/// `(metric, seed, u, v)`, from scanning 1.2M samples per metric against a
/// 9×9 search. Euclidean's misses are too rare for the sweep below to hit.
const SHORT_REACH_MISSES: [(VoronoiMetric, u64, f32, f32); 3] = [
    (Euclidean, 1167, 0.977_623_05, 0.400_408_63), // the wrong cell
    (Euclidean, 1543, 0.622_436_46, 0.187_380_08), // edges 0.16 off
    (Manhattan, 1321, 0.812_072_5, 0.579_942_5),   // the wrong cell
];

/// Every output checked unclamped where it can be: `Edges` against the true
/// `F2 − F1` and `Cells` against the true nearest cell's colour, so a reach too
/// short for either fails (forcing every reach to 1 fails both Euclidean misses
/// above and the sweep's Manhattan edges and cells).
#[test]
fn the_search_reach_finds_the_nearest_two_points_under_every_metric() {
    let sweep = METRICS.into_iter().flat_map(|metric| {
        (0..6).flat_map(move |seed| {
            (0..400).map(move |i| {
                let (u, v) = (hash::unit2(i, 0, 3, seed), hash::unit2(i, 1, 3, seed));
                (metric, seed, u, v)
            })
        })
    });
    for (metric, seed, u, v) in SHORT_REACH_MISSES.into_iter().chain(sweep) {
        let at = |output| sampler(5.0, output, metric, Field::Const(1.0), seed)(u, v);
        let (b1, b2, cell) = brute_force(5, metric, seed, u * 5.0, v * 5.0);
        assert!(
            (at(Distance)[0] - b1.min(1.0)).abs() < 1e-6,
            "{metric:?} F1"
        );
        assert!((at(F2)[0] - b2.min(1.0)).abs() < 1e-6, "{metric:?} F2");
        let gap = (b2 - b1).min(1.0);
        assert!((at(Edges)[0] - gap).abs() < 1e-5, "{metric:?} {seed} edges");
        let c = |ch| hash::unit2(cell.0, cell.1, ch, seed);
        assert_eq!(
            at(Cells),
            [c(3), c(4), c(5), 1.0],
            "{metric:?} {seed} cells"
        );
    }
}
