//! #1740's generators and params read the same at every resolution: a bake at 128²,
//! 512² and 2048² has the same mean and the same coarse histogram, as the older
//! generators do (`resolution.rs`). Each is a sampler over the unit tile, so this
//! holds unless a param slipped into pixels.

use rusty::procgen::{evaluate, Image, TextureRecipe};

/// One node per new kind or param, each set off its default.
const OPS: &[&str] = &[
    r#""op": "anisotropic_noise", "scale": 20, "anisotropy": 6, "angle": 0.15"#,
    r#""op": "wave", "kind": "bands", "frequency": 4, "profile": "saw""#,
    r#""op": "wave", "kind": "rings", "frequency": 4, "profile": "triangle""#,
    r#""op": "wave", "kind": "bands", "frequency": 3, "distortion": 6, "detail": 4"#,
    r#""op": "noise", "kind": "fbm", "scale": 4, "octaves": 3, "distortion": 2"#,
    r#""op": "voronoi", "scale": 5, "output": "smooth_f1", "smoothness": 0.7"#,
    r#""op": "shape", "kind": "polygon", "size": [0.7, 0.6], "sides": 5"#,
    r#""op": "shape", "kind": "star", "size": [1.3, 1.3], "points": 3, "inner_radius": 0.3"#,
    r#""op": "shape", "kind": "circle", "size": [0.8, 0.8], "profile": "cone""#,
    r#""op": "shape", "kind": "rect", "size": [0.8, 0.5], "profile": "paraboloid""#,
    r#""op": "shape", "kind": "line", "size": [0.9, 0.3], "profile": "hemisphere""#,
];

const BINS: usize = 8;

fn bake(op: &str, resolution: u32) -> Image {
    let json =
        format!(r#"{{"resolution": {resolution}, "seed": 2, "nodes": [{{"id": "n", {op}}}]}}"#);
    let recipe: TextureRecipe = serde_json::from_str(&json).expect("recipe parses");
    evaluate(&recipe).expect("recipe evaluates")
}

/// The red channel's mean and its share of pixels in each of [`BINS`] bins.
fn profile(img: &Image) -> (f32, [f32; BINS]) {
    let n = img.pixels().len() as f32;
    let mut bins = [0.0; BINS];
    let mut sum = 0.0;
    for p in img.pixels() {
        sum += p[0];
        bins[((p[0].clamp(0.0, 1.0) * BINS as f32) as usize).min(BINS - 1)] += 1.0 / n;
    }
    (sum / n, bins)
}

#[test]
fn each_new_generator_reads_the_same_at_128_512_and_2048() {
    for op in OPS {
        let (mean, bins) = profile(&bake(op, 2048));
        for res in [128, 512] {
            let (m, b) = profile(&bake(op, res));
            assert!(
                (m - mean).abs() < 0.01,
                "{op} at {res}²: mean {m} vs {mean}"
            );
            let moved: f32 = b.iter().zip(&bins).map(|(x, y)| (x - y).abs()).sum();
            assert!(moved < 0.05, "{op} at {res}²: histogram moved {moved}");
        }
    }
}
