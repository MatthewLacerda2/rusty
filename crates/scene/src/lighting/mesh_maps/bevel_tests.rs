//! Curvature follows the shape, not the tessellation (#1849 review): a right-angled
//! edge reads the same peak sharp, chamfered or rounded.

use std::f32::consts::FRAC_PI_2;

use glam::{Vec2, Vec3};

use super::curvature::Curvature;
use super::fixtures::quad;
use super::{Baked, MeshSource};
use crate::lighting::lightmap::TexelPoint;

/// The bevel's width beside the band (the band is 2 % of the √3 diagonal, ~0.035).
const BEVEL: f32 = 0.004;

/// A convex right-angled edge along z: a top face `y = 0` for `x ∈ [-1, 0]`, a side
/// `x = 0` for `y ∈ [-1, 0]`, the corner rounded in `segments` flat steps
/// (0 sharp, 1 a 45° chamfer). Returns the mesh and its profile.
fn edge(segments: u32) -> (MeshSource, Vec<Vec2>) {
    let mut profile = vec![Vec2::new(-1.0, 0.0)];
    if segments == 0 {
        profile.push(Vec2::ZERO);
    } else {
        for i in 0..=segments {
            let phi = FRAC_PI_2 * (1.0 - i as f32 / segments as f32);
            profile.push(Vec2::splat(-BEVEL) + BEVEL * Vec2::new(phi.cos(), phi.sin()));
        }
    }
    profile.push(Vec2::new(0.0, -1.0));
    let mut m = MeshSource::default();
    let w = 1.0 / (profile.len() - 1) as f32;
    for (k, s) in profile.windows(2).enumerate() {
        let at = |p: Vec2, z: f32| Vec3::new(p.x, p.y, z);
        let c = [at(s[0], 0.0), at(s[0], 1.0), at(s[1], 1.0), at(s[1], 0.0)];
        let uv = (
            Vec2::new(k as f32 * w, 0.0),
            Vec2::new((k + 1) as f32 * w, 1.0),
        );
        quad(&mut m, c, uv, 0);
    }
    (m, profile)
}

/// The brightest curvature anywhere across the profile, at mid-length.
fn peak(segments: u32) -> f32 {
    let (source, profile) = edge(segments);
    let (min, max) = (Vec3::new(-1.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
    let baked = Baked {
        source: &source,
        size: 1024,
        points: Vec::new(),
        min,
        extent: max - min,
        diagonal: (max - min).length(),
    };
    let curvature = Curvature::new(&baked);
    let mut best = 0.0f32;
    for (k, s) in profile.windows(2).enumerate() {
        for t in 0..=400 {
            let p = s[0].lerp(s[1], t as f32 / 400.0);
            let point = TexelPoint {
                index: 0,
                position: Vec3::new(p.x, p.y, 0.5),
                normal: source.normals[4 * k],
                first: 6 * k as u32,
            };
            best = best.max(curvature.texel(&point).x);
        }
    }
    best
}

#[test]
fn a_right_angle_peaks_alike_sharp_chamfered_and_rounded() {
    let sharp = peak(0);
    assert!((sharp - 1.0).abs() < 1e-3, "sharp edge peaks at {sharp}");
    for segments in [1, 3, 8] {
        let bevel = peak(segments);
        // The bevel is ~an eighth of the band wide: its creases sit that far apart.
        assert!(
            (bevel - sharp).abs() < 0.05,
            "{segments}-segment bevel peaks at {bevel}, sharp at {sharp}"
        );
    }
}
