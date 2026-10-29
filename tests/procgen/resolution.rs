//! #394's guard, and the definition of "resolution-independent": a recipe baked at
//! 128² must match the same recipe baked at 512² and box-downsampled to 128². Every
//! op whose params used to be in pixels (`blur`, `bump_to_normal`) or that resampled
//! nearest-neighbour (`mapping`) is exercised.

use rusty::procgen::recipe::{
    Blur, BlurKind, BumpToNormal, Cavity, Checker, Curvature, Mapping, Node, Noise, NoiseKind,
    OpKind,
};
use rusty::procgen::runner::stats::node_stats;
use rusty::procgen::Image;

use super::node;

fn bake(nodes: &[Node], resolution: u32) -> Image {
    super::bake(nodes.to_vec(), resolution, 3)
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
        OpKind::Noise(Noise {
            kind: NoiseKind::Perlin,
            scale,
            octaves: 1,
            lacunarity: 2.0,
            gain: 0.5,
            seed: 0,
        }),
        &[],
    )
}

#[test]
fn blur_radius_is_a_fraction_of_the_tile() {
    let checker = OpKind::Checker(Checker {
        tiles: 4,
        color_a: [0.0; 4],
        color_b: [1.0; 4],
    });
    for kind in [BlurKind::Box, BlurKind::Gaussian] {
        let blur = OpKind::Blur(Blur { radius: 0.03, kind });
        let nodes = [node("k", checker.clone(), &[]), node("b", blur, &["k"])];
        assert_resolution_independent(&format!("blur {kind:?}"), &nodes, 0.02);
    }
}

#[test]
fn bump_to_normal_strength_is_a_slope_in_tile_units() {
    let nodes = [
        perlin(4.0),
        node(
            "b",
            OpKind::BumpToNormal(BumpToNormal { strength: 0.3 }),
            &["n"],
        ),
    ];
    assert_resolution_independent("bump_to_normal", &nodes, 0.02);
}

#[test]
fn mapping_resamples_bilinearly() {
    for tiling in [true, false] {
        let map = OpKind::Mapping(Mapping {
            scale: [1.7, 1.3],
            rotation: 0.1,
            translation: [0.2, 0.0],
            tiling,
        });
        let nodes = [perlin(3.0), node("m", map, &["n"])];
        assert_resolution_independent(&format!("mapping tiling={tiling}"), &nodes, 0.01);
    }
}

#[test]
fn height_op_radii_are_in_tile_units() {
    let ops = [
        OpKind::Cavity(Cavity { radius: 0.04 }),
        OpKind::Curvature(Curvature {
            radius: 0.03,
            strength: 3.0,
        }),
    ];
    for op in ops {
        let label = format!("{op:?}");
        let nodes = [perlin(4.0), node("c", op, &["n"])];
        assert_resolution_independent(&label, &nodes, 0.02);
    }
}

/// `Texture.Stats` at 128² must read like 1024²: a gaussian too narrow for a pass
/// at the low size (`0.01 × 128 ≈ 1.3 px`) falls back to one radius-1 box rather
/// than turning `cavity` flat white.
#[test]
fn a_small_cavity_reads_alike_at_128_and_1024() {
    let nodes = [
        perlin(16.0),
        node("c", OpKind::Cavity(Cavity { radius: 0.01 }), &["n"]),
    ];
    let depth = |res| {
        let s = node_stats(&bake(&nodes, res), &mut Vec::new());
        (1.0 - s.mean[0], 1.0 - s.p1[0])
    };
    let (low, high) = (depth(128), depth(1024));
    for (l, h) in [(low.0, high.0), (low.1, high.1)] {
        assert!(
            l > 0.0 && (l / h - 1.0).abs() < 0.2,
            "128² {l} vs 1024² {h}"
        );
    }
}
