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
        let at = |output| sampler(4.0, output, metric, 0.0, 9)(u, v)[0];
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
/// `cells`² tile and its eight wrapped copies: what the sampler must find.
fn brute_force(cells: i64, metric: VoronoiMetric, seed: u64, px: f32, py: f32) -> (f32, f32) {
    let mut d: Vec<f32> = Vec::new();
    for cy in -cells..2 * cells {
        for cx in -cells..2 * cells {
            let (wx, wy) = (cx.rem_euclid(cells), cy.rem_euclid(cells));
            let at = |ch| hash::unit2(wx, wy, ch, seed);
            d.push(measure(
                metric,
                cx as f32 + at(1) - px,
                cy as f32 + at(2) - py,
            ));
        }
    }
    d.sort_by(f32::total_cmp);
    (d[0], d[1])
}

#[test]
fn the_search_reach_finds_the_nearest_two_points_under_every_metric() {
    let cells = 5;
    for metric in METRICS {
        for seed in 0..6 {
            let f1 = sampler(cells as f32, Distance, metric, 1.0, seed);
            let f2 = sampler(cells as f32, F2, metric, 1.0, seed);
            for i in 0..400 {
                let (u, v) = (hash::unit2(i, 0, 3, seed), hash::unit2(i, 1, 3, seed));
                let (b1, b2) = brute_force(cells, metric, seed, u * 5.0, v * 5.0);
                assert!((f1(u, v)[0] - b1.min(1.0)).abs() < 1e-6, "{metric:?} F1");
                assert!((f2(u, v)[0] - b2.min(1.0)).abs() < 1e-6, "{metric:?} F2");
            }
        }
    }
}
