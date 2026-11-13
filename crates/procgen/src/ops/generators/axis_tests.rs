//! A generator's `aspect` (#2002): `scale` per axis, each a whole count.

use super::super::field::Field;
use super::{noise, voronoi};
use crate::recipe::{NoiseKind, VoronoiMetric, VoronoiOutput};

#[test]
fn perlin_counts_its_lattice_per_axis() {
    // Perlin is 0 (mid-grey) on every lattice point: at aspect {1, 2} they sit 4
    // across and 8 down, and halfway down between two of them it is not 0.
    let gain = (1, 2.0, Field::Const(0.5));
    let n = noise::sampler(NoiseKind::Perlin, [4.0, 8.0], gain, 0.0, 7);
    for (i, j) in [(0, 0), (1, 3), (3, 7), (2, 5)] {
        assert_eq!(n(i as f32 / 4.0, j as f32 / 8.0)[0], 0.5, "({i}, {j})");
    }
    let off = (0..8).filter(|j| n(0.125, (*j as f32 + 0.5) / 8.0)[0] != 0.5);
    assert!(off.count() > 0);
}

#[test]
fn voronoi_counts_its_cells_per_axis() {
    // A regular grid (randomness 0) puts each point at its cell's centre, where
    // the distance is 0.
    let modes = (VoronoiOutput::Distance, VoronoiMetric::Euclidean);
    let v = voronoi::sampler([3.0, 6.0], modes, Field::Const(0.0), 1.0, 2);
    for (i, j) in [(0, 0), (2, 5), (1, 3)] {
        let (u, w) = ((i as f32 + 0.5) / 3.0, (j as f32 + 0.5) / 6.0);
        assert!(v(u, w)[0] < 1e-5, "({i}, {j})");
    }
    // Between two centres down, half a cell from each.
    assert!((v(0.5 / 3.0, 1.0 / 6.0)[0] - 0.5).abs() < 1e-5);
}

#[test]
fn gray_cells_are_one_value_per_cell() {
    use crate::recipe::Voronoi;
    let mut p: Voronoi =
        serde_json::from_value(serde_json::json!({ "scale": 4, "output": "cells" })).unwrap();
    let colour = super::voronoi(&p, Field::Const(1.0), 3);
    p.grayscale = true;
    let gray = super::voronoi(&p, Field::Const(1.0), 3);
    for (u, v) in [(0.1, 0.2), (0.6, 0.9), (0.33, 0.71)] {
        let (c, g) = (colour(u, v), gray(u, v));
        assert_eq!(g, [c[0], c[0], c[0], 1.0]);
        assert_ne!(c[0], c[1], "the colour cells are not gray");
    }
}
