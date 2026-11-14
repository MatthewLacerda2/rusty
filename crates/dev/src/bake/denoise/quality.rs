//! The error measure behind `quality_tests` (#1884): a bake's lightmaps against a
//! converged one's, in luminance, over each chart's interior (a texel whose four
//! neighbours are on its chart too). The gutter between charts and the ring
//! dilation fills are copies no surface shows; counted, they swamped the noise in
//! the first, all-texel measure.

use glam::{Vec2, Vec3};

use rusty_core::color::luminance;
use rusty_lighting::lightmap::{BakeMesh, BakeScene, Lightmap};

/// Whether each texel of `mesh`'s `size`² lightmap is inside its chart, four
/// neighbours included.
fn interior(mesh: &BakeMesh, size: u32) -> Vec<bool> {
    let n = size as usize;
    let mut on = vec![false; n * n];
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.lightmap_uvs[i as usize] * size as f32);
        let cross = |o: Vec2, p: Vec2, q: Vec2| (p - o).perp_dot(q - o);
        if cross(a, b, c) == 0.0 {
            continue;
        }
        for (k, texel) in on.iter_mut().enumerate() {
            let p = Vec2::new((k % n) as f32 + 0.5, (k / n) as f32 + 0.5);
            let w = [cross(b, c, p), cross(c, a, p), cross(a, b, p)];
            *texel |= w.iter().all(|w| *w >= 0.0) || w.iter().all(|w| *w <= 0.0);
        }
    }
    let at = |x: usize, y: usize| on[y * n + x];
    (0..n * n)
        .map(|k| {
            let (x, y) = (k % n, k / n);
            let inner = x > 0 && y > 0 && x + 1 < n && y + 1 < n;
            inner && at(x, y) && at(x - 1, y) && at(x + 1, y) && at(x, y - 1) && at(x, y + 1)
        })
        .collect()
}

/// How far a bake is from the reference, over every chart interior, in luminance.
pub struct Error {
    /// Mean absolute error over the reference's mean.
    pub mae: f64,
    /// The mean of each texel's own relative error, `|v − r| / (r + 1% of the
    /// mean)`: a dim corner counts as much as the pool under a lamp, as the eye
    /// sees it.
    pub relative: f64,
}

/// `maps` against `reference`, chart by chart.
pub fn error(scene: &BakeScene, maps: &[Lightmap], reference: &[Lightmap]) -> Error {
    let mut pairs = Vec::new();
    for (map, truth) in maps.iter().zip(reference) {
        assert_eq!(map.entity, truth.entity, "the same charts in order");
        let mesh = scene.meshes.iter().find(|m| m.entity == map.entity);
        let inside = interior(mesh.expect("the baked mesh"), map.size);
        let both = map.texels.iter().zip(&truth.texels).zip(inside);
        let luma = |(v, r): (&Vec3, &Vec3)| (luminance(*v) as f64, luminance(*r) as f64);
        pairs.extend(
            both.filter(|(_, inside)| *inside)
                .map(|(pair, _)| luma(pair)),
        );
    }
    let n = pairs.len() as f64;
    let mean = pairs.iter().map(|(_, r)| r).sum::<f64>() / n;
    let average =
        |f: &dyn Fn(f64, f64) -> f64| pairs.iter().map(|&(v, r)| f(v, r)).sum::<f64>() / n;
    Error {
        mae: average(&|v, r| (v - r).abs()) / mean,
        relative: average(&|v, r| (v - r).abs() / (r + 0.01 * mean)),
    }
}
