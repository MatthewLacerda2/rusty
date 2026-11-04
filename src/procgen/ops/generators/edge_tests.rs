//! #1740's review cases: a count past any bake, a linked gain under a warp, an
//! elongated dome, and fibres on an axis.

use super::super::*;
use crate::procgen::catalog::generators::shape::ShapeProfile;
use crate::procgen::catalog::generators::wave::WaveProfile;
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::{
    AnisotropicNoise, NoiseKind, ShapeKind, VoronoiMetric, VoronoiOutput,
};
use crate::procgen::recipe::{Wave, WaveKind};

fn fbm(scale: f32, distortion: f32) -> Noise {
    let (kind, octaves, lacunarity, gain, seed) = (NoiseKind::Fbm, 4, 2.0, 0.5.into(), 0);
    Noise {
        kind,
        scale,
        octaves,
        lacunarity,
        gain,
        distortion,
        seed,
    }
}

#[test]
fn a_huge_or_malformed_count_is_capped_not_a_panic() {
    assert_eq!(count(f32::NAN), 1.0);
    assert_eq!(count(f32::INFINITY), MAX_COUNT);
    assert_eq!(count(1e30), MAX_COUNT);
    assert_eq!(count(2048.0), 2048.0, "a real count is left alone");
    for x in [1e30, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let (profile, distortion, detail, seed) = (WaveProfile::Sine, 1.0, 2, 0);
        let wave_op = Wave {
            kind: WaveKind::Bands,
            frequency: x,
            profile,
            distortion,
            detail,
            seed,
        };
        let vor = Voronoi {
            scale: x,
            output: VoronoiOutput::Distance,
            metric: VoronoiMetric::Euclidean,
            randomness: 1.0.into(),
            smoothness: 1.0,
            seed: 0,
        };
        let samplers = [
            noise(&fbm(x, 0.0), Field::Const(0.5), 1),
            wave(&wave_op, 1),
            voronoi(&vor, Field::Const(1.0), 1),
        ];
        for f in &samplers {
            for (u, v) in [(0.0, 0.0), (0.3, 0.7), (0.999, 0.5)] {
                let c = f(u, v)[0];
                assert!((0.0..=1.0).contains(&c), "count {x}: {c}");
            }
        }
    }
}

#[test]
fn a_linked_gain_under_distortion_has_no_seam() {
    // Red 0 on the two edge columns and 1 between: the same at both sides of the
    // wrap, but a read pushed past an edge clamps onto a different column.
    let img = Image::fill_uv(64, |u, _| {
        let edge = !(1.0 / 64.0..63.0 / 64.0).contains(&u);
        [if edge { 0.0 } else { 1.0 }, 0.0, 0.0, 1.0]
    });
    let gain = Field::Map {
        img: &img,
        min: 0.0,
        max: 1.0,
    };
    let f = noise(&fbm(4.0, 2.0), gain, 3);
    for i in 0..64 {
        let t = (i as f32 + 0.5) / 64.0;
        let (a, b) = (f(1e-4, t)[0], f(1.0 - 1e-4, t)[0]);
        assert!((a - b).abs() < 2e-3, "seam at v={t}: {a} vs {b}");
    }
}

#[test]
fn a_non_round_dome_has_one_peak_at_the_centre() {
    let p = Shape {
        kind: ShapeKind::Circle,
        size: [0.8, 0.4],
        roundness: 0.25,
        softness: 0.0.into(),
        sides: 6,
        points: 5,
        inner_radius: 0.5,
        profile: ShapeProfile::Hemisphere,
    };
    let f = shape(&p, Field::Const(0.0));
    assert!((f(0.5, 0.5)[0] - 1.0).abs() < 1e-6);
    let mut last = 1.0;
    for i in 1..=7 {
        let h = f(0.5 + i as f32 * 0.05, 0.5)[0];
        assert!(
            h < last,
            "the dome falls along the major axis: {h} at step {i}"
        );
        last = h;
    }
    assert_eq!(f(0.95, 0.5)[0], 0.0, "and is 0 past the edge");
}

#[test]
fn fibres_on_an_axis_vary_along_their_length() {
    for scale in [8.0, 1.0] {
        for angle in [0.0, 0.25] {
            let op = AnisotropicNoise {
                scale,
                anisotropy: 8.0,
                angle,
                seed: 0,
            };
            let f = anisotropic::sampler(&op, 2);
            // Along the fibre is along `u` at angle 0, along `v` at 0.25.
            let along = |s: f32| {
                if angle == 0.0 {
                    f(s, 0.3)[0]
                } else {
                    f(0.3, s)[0]
                }
            };
            let values: Vec<f32> = (0..32).map(|i| along(i as f32 / 32.0)).collect();
            let (lo, hi) = values
                .iter()
                .fold((1f32, 0f32), |(a, b), &x| (a.min(x), b.max(x)));
            assert!(
                hi - lo > 0.05,
                "scale {scale}, angle {angle}: flat along the fibre"
            );
        }
    }
}
