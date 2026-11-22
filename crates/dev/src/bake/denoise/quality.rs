use glam::{Vec2, Vec3};

use rusty_core::color::luminance;
use rusty_lighting::lightmap::{BakeMesh, BakeScene, Lightmap};

// Chart interiors only: the gutter and the dilation's fill are copies no surface shows.
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

pub struct Error {
    pub mae: f64,
    pub relative: f64,
}

pub fn error(scene: &BakeScene, maps: &[Lightmap], reference: &[Lightmap]) -> Error {
    let mut pairs = Vec::new();
    for (map, truth) in maps.iter().zip(reference) {
        assert_eq!(map.entity, truth.entity, "the same charts in order");
        let mesh = scene.meshes.iter().find(|m| m.entity == map.entity);
        let inside = interior(mesh.expect("the baked mesh"), map.size);
        let both = map.texels().into_iter().zip(truth.texels()).zip(inside);
        let luma = |(v, r): (Vec3, Vec3)| (luminance(v) as f64, luminance(r) as f64);
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
        // 1% of the mean added to the reference keeps a black texel's relative error finite.
        relative: average(&|v, r| (v - r).abs() / (r + 0.01 * mean)),
    }
}
