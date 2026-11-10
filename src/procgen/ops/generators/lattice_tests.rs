//! The lattice generators the periodicity test runs beyond noise and Voronoi's
//! basics: #407's hard-surface stamps and #1740's fibres, profiles and distortions.

use super::super::*;
use crate::procgen::recipe::ShapeKind::*;
use crate::procgen::recipe::{AnisotropicNoise, BrickOutput, NoiseKind, OpKind, Wave, WaveKind};

/// #407's brick outputs and shapes at fractional `s`.
pub(super) fn hard_surface_ops(s: f32) -> Vec<OpKind> {
    let mut ops = Vec::new();
    for output in [BrickOutput::Mask, BrickOutput::Random, BrickOutput::Bevel] {
        let (rows, cols, mortar) = (s, s + 0.7, 0.05);
        ops.push(OpKind::Brick(Brick {
            rows,
            cols,
            mortar,
            output,
            seed: 0,
        }));
    }
    for kind in [Circle, Rect, RoundedRect, Line, Polygon, Star] {
        let (size, roundness, softness) = ([s / 6.0, 0.3], 0.5, s / 20.0);
        ops.push(OpKind::Shape(Shape {
            kind,
            size,
            roundness,
            softness: softness.into(),
            sides: 6,
            points: 5,
            inner_radius: 0.5,
            profile: Default::default(),
        }));
    }
    ops
}

/// #1740's fibres, wave profiles and distortions at fractional `s`.
pub(super) fn distorted_ops(s: f32) -> Vec<OpKind> {
    use crate::procgen::catalog::generators::wave::WaveProfile::*;
    let mut ops = vec![OpKind::AnisotropicNoise(AnisotropicNoise {
        scale: s * 3.0,
        anisotropy: s,
        angle: s / 10.0,
        seed: 0,
    })];
    for (profile, distortion) in [(Sine, 0.0), (Sine, s), (Saw, s), (Triangle, s)] {
        let (detail, seed) = (3, 0);
        let (kind, frequency) = (WaveKind::Bands, s);
        ops.push(OpKind::Wave(Wave {
            kind,
            frequency,
            profile,
            distortion,
            detail,
            seed,
        }));
    }
    let (gain, distortion) = (0.5.into(), s / 2.0);
    let (kind, scale, octaves, lacunarity, seed) = (NoiseKind::Fbm, s, 3, 2.0, 0);
    ops.push(OpKind::Noise(Noise {
        kind,
        scale,
        aspect: [1.0, 1.0],
        octaves,
        lacunarity,
        gain,
        distortion,
        seed,
    }));
    ops
}
