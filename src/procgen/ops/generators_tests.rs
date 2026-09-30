//! The periodicity property behind "seamless by construction" (#392), tested on the
//! samplers themselves: a lattice generator's value at `(u, v)` equals its value one
//! whole tile away, for fractional params too (they round to whole counts).

use super::*;

/// A spread of sample points, deterministic and off every lattice line.
fn points() -> impl Iterator<Item = (f32, f32)> {
    (0..97).map(|i| {
        (
            hash::unit2(i, 0, 11, 3) * 0.998,
            hash::unit2(i, 1, 11, 3) * 0.998,
        )
    })
}

fn close(a: Rgba, b: Rgba) -> bool {
    (0..4).all(|c| (a[c] - b[c]).abs() < 2e-3)
}

/// Every generator whose pattern is a whole-tile lattice, over fractional params.
fn lattice_ops() -> Vec<OpKind> {
    let mut ops = vec![OpKind::WhiteNoise];
    for s in [0.4, 1.0, 2.5, 4.5, 5.5, 7.3] {
        for kind in [NoiseKind::Perlin, NoiseKind::Fbm] {
            ops.push(OpKind::Noise {
                kind,
                scale: s,
                octaves: 3,
            });
        }
        for output in [VoronoiOutput::Distance, VoronoiOutput::Cells] {
            ops.push(OpKind::Voronoi { scale: s, output });
        }
        ops.push(OpKind::Wave {
            kind: WaveKind::Bands,
            frequency: s,
        });
        ops.push(OpKind::Brick {
            rows: s,
            cols: s + 0.7,
            mortar: 0.05,
        });
        ops.push(OpKind::Checker {
            tiles: s as u32,
            color_a: [0.0; 4],
            color_b: [1.0; 4],
        });
    }
    ops.push(OpKind::Gradient {
        kind: GradientKind::LinearTiling,
    });
    ops
}

#[test]
fn lattice_generators_repeat_every_whole_tile() {
    for op in lattice_ops() {
        let f = sampler(&op, 64, 9).expect("generator");
        for (u, v) in points() {
            let here = f(u, v);
            for (du, dv) in [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let there = f(u + du, v + dv);
                assert!(
                    close(here, there),
                    "{op:?} at ({u},{v})+({du},{dv}): {here:?} vs {there:?}"
                );
            }
        }
    }
}

#[test]
fn radial_generators_meet_the_edge_continuously() {
    let radial = [
        OpKind::Gradient {
            kind: GradientKind::Radial,
        },
        OpKind::Wave {
            kind: WaveKind::Rings,
            frequency: 3.4,
        },
    ];
    for op in radial {
        let f = sampler(&op, 64, 0).expect("generator");
        for (t, _) in points() {
            let (lo, hi) = (1e-4, 1.0 - 1e-4);
            assert!(close(f(lo, t), f(hi, t)), "{op:?} left/right edge at v={t}");
            assert!(close(f(t, lo), f(t, hi)), "{op:?} top/bottom edge at u={t}");
        }
    }
}

#[test]
fn counts_round_to_whole_periods_and_brick_rows_to_even() {
    assert_eq!(count(4.5), 5.0);
    assert_eq!(count(4.4), 4.0);
    assert_eq!(count(0.2), 1.0);
    // An odd checker rounds up to even: 3 tiles paint as 4.
    let k = sampler(
        &OpKind::Checker {
            tiles: 3,
            color_a: [0.0; 4],
            color_b: [1.0; 4],
        },
        8,
        0,
    );
    assert_eq!(
        k.unwrap()(0.3, 0.1)[0],
        1.0,
        "u = 0.3 is square 1 of 4, not 0 of 3"
    );
    // 3 rows round to 4, so the half-brick offset alternation closes at the wrap.
    let f = sampler(
        &OpKind::Brick {
            rows: 3.0,
            cols: 2.0,
            mortar: 0.1,
        },
        64,
        0,
    )
    .unwrap();
    // v = 0.3 is row 1 of 4 (odd, offset half a brick); with 3 rows it'd be row 0.
    assert_eq!(f(0.5, 0.3)[0], 1.0);
    assert_eq!(f(0.76, 0.2)[0], 1.0, "row 0 is not offset");
    assert_eq!(f(0.76, 0.3)[0], 0.0, "row 1 is offset half a brick");
}

#[test]
fn linear_tiling_gradient_is_a_triangle() {
    let f = sampler(
        &OpKind::Gradient {
            kind: GradientKind::LinearTiling,
        },
        8,
        0,
    )
    .unwrap();
    assert_eq!(f(0.0, 0.3)[0], 0.0);
    assert_eq!(f(0.5, 0.3)[0], 1.0);
    assert!((f(0.25, 0.3)[0] - 0.5).abs() < 1e-6);
    assert!((f(0.75, 0.3)[0] - 0.5).abs() < 1e-6);
}
