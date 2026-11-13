//! The nearest-probe index returns exactly what the full scan does (#1737): the
//! same probes, in the same order, ties to the lower index.

use glam::Vec3;

use super::{scan, ProbeIndex};
use crate::lighting::probe::Probe;

/// A deterministic scatter of `n` points in `[0, size)` (an LCG, no RNG needed).
fn scatter(n: usize, size: Vec3) -> Vec<Probe> {
    let mut x: u32 = 12345;
    let mut next = move || {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (x >> 8) as f32 / (1u32 << 24) as f32
    };
    (0..n)
        .map(|_| Probe::new(Vec3::new(next(), next(), next()) * size))
        .collect()
}

/// Queries inside, on the edge of and far outside `size`, on a lattice that also
/// lands on probes of a lattice set.
fn queries(size: Vec3) -> impl Iterator<Item = Vec3> {
    (-2..=12).flat_map(move |i| {
        (-2..=12).flat_map(move |k| {
            [-30.0, 0.5, 3.0].map(move |y| Vec3::new(i as f32, y, k as f32) * size / 10.0)
        })
    })
}

fn assert_same(probes: &[Probe], size: Vec3) {
    let index = ProbeIndex::build(probes).expect("big enough to index");
    for at in queries(size).chain(probes.iter().map(|p| p.position)) {
        assert_eq!(index.nearest(probes, at), scan(probes, at), "at {at}");
    }
}

#[test]
fn a_scatter_matches_the_scan() {
    let size = Vec3::new(200.0, 40.0, 200.0);
    assert_same(&scatter(3000, size), size);
}

/// A flat set grids in 2D: a query in it looks at a few cubes, never the
/// millimetre grid a zero-height cube would make, nor a scan.
#[test]
fn a_flat_layout_matches_the_scan() {
    let size = Vec3::new(50.0, 0.0, 30.0);
    let probes = scatter(500, size);
    assert_same(&probes, size);
    let index = ProbeIndex::build(&probes).unwrap();
    let inside = |q: &Vec3| q.cmpge(Vec3::ZERO).all() && q.cmple(size).all();
    for at in queries(size).filter(inside) {
        let work = index.nearest_counted(&probes, at).1;
        assert!(work.is_some_and(|w| w < 64), "{work:?} cubes at {at}");
    }
}

/// A lattice queried at its cells' middles has many probes at the same distance:
/// the index breaks the ties as the scan does.
#[test]
fn ties_go_to_the_lower_index_as_in_the_scan() {
    let mut probes = Vec::new();
    for z in 0..8 {
        for y in 0..3 {
            for x in 0..8 {
                probes.push(Probe::new(Vec3::new(x as f32, y as f32, z as f32) * 2.0));
            }
        }
    }
    let index = ProbeIndex::build(&probes).unwrap();
    for at in [
        Vec3::splat(1.0),
        Vec3::new(5.0, 3.0, 7.0),
        Vec3::new(9.0, 1.0, 3.0),
    ] {
        assert_eq!(index.nearest(&probes, at), scan(&probes, at), "at {at}");
    }
}

/// Small sets, non-finite positions and a stale index all fall back to the scan.
#[test]
fn what_it_cannot_index_it_scans() {
    assert!(ProbeIndex::build(&scatter(8, Vec3::ONE)).is_none(), "small");
    let mut probes = scatter(64, Vec3::splat(10.0));
    probes[3].position = Vec3::NAN;
    probes[5].position = Vec3::INFINITY;
    let index = ProbeIndex::build(&probes).unwrap();
    for at in [Vec3::splat(5.0), Vec3::NAN, Vec3::splat(1.0e30)] {
        assert_eq!(index.nearest(&probes, at), scan(&probes, at), "at {at}");
    }
    probes.push(Probe::new(Vec3::splat(5.0)));
    let at = Vec3::splat(5.0);
    assert_eq!(index.nearest(&probes, at), scan(&probes, at), "stale");
}
