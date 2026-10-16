//! #394's guard, and the definition of "resolution-independent": a recipe baked at
//! 128² must match the same recipe baked at 512² and box-downsampled to 128². Every
//! op whose params used to be in pixels (`blur`, `bump_to_normal`) or that resampled
//! nearest-neighbour (`mapping`) is exercised.

use rusty::procgen::recipe::{Node, NoiseKind, OpKind, TextureRecipe};
use rusty::procgen::{evaluate, Image};

pub fn node(id: &str, op: OpKind, inputs: &[&str]) -> Node {
    let inputs = inputs.iter().map(|s| s.to_string()).collect();
    Node {
        id: id.into(),
        op,
        inputs,
    }
}

fn bake(nodes: &[Node], resolution: u32) -> Image {
    let nodes = nodes.to_vec();
    let recipe = TextureRecipe::new(resolution, nodes).with_seed(3);
    evaluate(&recipe).expect("recipe evaluates")
}

/// Average each `k`×`k` block of `img` into one pixel.
fn downsample(img: &Image, k: u32) -> Image {
    let res = img.resolution() / k;
    let mut out = Image::new(res);
    let inv = 1.0 / (k * k) as f32;
    for (i, px) in out.pixels_mut().iter_mut().enumerate() {
        let (bx, by) = ((i as u32 % res) * k, (i as u32 / res) * k);
        let mut acc = [0.0f32; 4];
        for (x, y) in (0..k).flat_map(|y| (0..k).map(move |x| (x, y))) {
            let p = img.get_wrapped((bx + x) as i64, (by + y) as i64);
            (0..4).for_each(|c| acc[c] += p[c] * inv);
        }
        *px = acc;
    }
    out
}

/// Mean absolute RGB difference between two same-size images.
fn mean_diff(a: &Image, b: &Image) -> f32 {
    let sum: f32 = a
        .pixels()
        .iter()
        .zip(b.pixels())
        .map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f32>())
        .sum();
    sum / (a.pixels().len() * 3) as f32
}

pub fn assert_resolution_independent(label: &str, nodes: &[Node], tolerance: f32) {
    let low = bake(nodes, 128);
    let high = downsample(&bake(nodes, 512), 4);
    let d = mean_diff(&low, &high);
    assert!(
        d < tolerance,
        "{label}: 128² vs 512²↓ differ by {d} (tolerance {tolerance})"
    );
}

fn perlin(scale: f32) -> Node {
    node(
        "n",
        OpKind::Noise {
            kind: NoiseKind::Perlin,
            scale,
            octaves: 1,
            lacunarity: 2.0,
            gain: 0.5,
        },
        &[],
    )
}

#[test]
fn blur_radius_is_a_fraction_of_the_tile() {
    let checker = OpKind::Checker {
        tiles: 4,
        color_a: [0.0; 4],
        color_b: [1.0; 4],
    };
    let nodes = [
        node("k", checker, &[]),
        node("b", OpKind::Blur { radius: 0.03 }, &["k"]),
    ];
    assert_resolution_independent("blur", &nodes, 0.02);
}

#[test]
fn bump_to_normal_strength_is_a_slope_in_tile_units() {
    let nodes = [
        perlin(4.0),
        node("b", OpKind::BumpToNormal { strength: 0.3 }, &["n"]),
    ];
    assert_resolution_independent("bump_to_normal", &nodes, 0.02);
}

#[test]
fn mapping_resamples_bilinearly() {
    for tiling in [true, false] {
        let map = OpKind::Mapping {
            scale: [1.7, 1.3],
            rotation: 0.1,
            translation: [0.2, 0.0],
            tiling,
        };
        let nodes = [perlin(3.0), node("m", map, &["n"])];
        assert_resolution_independent(&format!("mapping tiling={tiling}"), &nodes, 0.01);
    }
}

#[test]
fn height_op_radii_are_in_tile_units() {
    let ops = [
        OpKind::Cavity { radius: 0.04 },
        OpKind::Curvature {
            radius: 0.03,
            strength: 3.0,
        },
    ];
    for op in ops {
        let label = format!("{op:?}");
        let nodes = [perlin(4.0), node("c", op, &["n"])];
        assert_resolution_independent(&label, &nodes, 0.02);
    }
}
