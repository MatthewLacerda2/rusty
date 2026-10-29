//! #392's guard: every generator (and `mapping`) bakes **seamless** — the jump across
//! the wrap (last column → first column, last row → first row) is no bigger than the
//! biggest jump between two neighbouring pixels inside the tile. Params deliberately
//! include fractional counts, the values that used to open a seam.

use rusty::procgen::recipe::{
    Brick, Cavity, Curvature, Gradient, GradientKind, Mapping, Node, Noise, NoiseKind, OpKind,
    Voronoi, VoronoiMetric, Wave,
};
use rusty::procgen::recipe::{NoiseKind::*, VoronoiOutput::*, WaveKind};
use rusty::procgen::Image;

pub use super::node;

const RES: u32 = 64;

pub fn bake(nodes: Vec<Node>) -> Image {
    super::bake(nodes, RES, 7)
}

/// Biggest per-channel RGB difference between two pixels.
fn jump(a: [f32; 4], b: [f32; 4]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

/// (seam jump, biggest interior neighbour jump) along x (`horizontal`) or y.
pub fn seam_and_interior(img: &Image, horizontal: bool) -> (f32, f32) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let (n, m) = if horizontal { (w, h) } else { (h, w) };
    let at = |i: i64, j: i64| {
        if horizontal {
            img.get_wrapped(i, j)
        } else {
            img.get_wrapped(j, i)
        }
    };
    let (mut seam, mut interior) = (0.0f32, 0.0f32);
    for j in 0..m {
        seam = seam.max(jump(at(n - 1, j), at(0, j)));
        for i in 0..n - 1 {
            interior = interior.max(jump(at(i, j), at(i + 1, j)));
        }
    }
    (seam, interior)
}

pub fn assert_seamless(label: &str, img: &Image) {
    for horizontal in [true, false] {
        let (seam, interior) = seam_and_interior(img, horizontal);
        assert!(
            seam <= interior + 1e-4,
            "{label}: seam jump {seam} exceeds interior bound {interior} (horizontal={horizontal})"
        );
    }
}

pub fn generators() -> Vec<(String, OpKind)> {
    let mut ops = Vec::new();
    for s in [1.0, 3.0, 4.5, 5.5, 7.3] {
        for kind in [Perlin, Fbm, Ridged, Turbulence] {
            ops.push((
                format!("noise {kind:?} {s}"),
                OpKind::Noise(Noise {
                    kind,
                    scale: s,
                    octaves: 3,
                    lacunarity: s,
                    gain: 0.5,
                    seed: 0,
                }),
            ));
        }
        for output in [Distance, F2, Edges] {
            let randomness = s / 8.0;
            let op = OpKind::Voronoi(Voronoi {
                scale: s,
                output,
                metric: VoronoiMetric::Euclidean,
                randomness,
                seed: 0,
            });
            ops.push((format!("voronoi {output:?} {s}"), op));
        }
        for kind in [WaveKind::Bands, WaveKind::Rings] {
            ops.push((
                format!("wave {kind:?} {s}"),
                OpKind::Wave(Wave { kind, frequency: s }),
            ));
        }
    }
    for kind in [GradientKind::Radial, GradientKind::LinearTiling] {
        ops.push((
            format!("gradient {kind:?}"),
            OpKind::Gradient(Gradient { kind }),
        ));
    }
    ops
}

#[test]
fn every_generator_bakes_without_a_seam() {
    for (label, op) in generators() {
        assert_seamless(&label, &bake(vec![node("g", op, &[])]));
    }
}

#[test]
fn mapping_keeps_a_seamless_input_seamless() {
    for (scale, rotation) in [([1.5, 2.3], 0.0), ([1.0, 1.0], 0.1), ([2.0, 0.7], 0.3)] {
        let src = OpKind::Noise(Noise {
            kind: NoiseKind::Perlin,
            scale: 3.0,
            octaves: 1,
            lacunarity: 2.0,
            gain: 0.5,
            seed: 0,
        });
        let map = OpKind::Mapping(Mapping {
            scale,
            rotation,
            translation: [0.13, 0.4],
            tiling: true,
        });
        let img = bake(vec![node("n", src, &[]), node("m", map, &["n"])]);
        assert_seamless(&format!("mapping {scale:?} {rotation}"), &img);
    }
}

#[test]
fn height_ops_keep_a_seamless_height_seamless() {
    let height = OpKind::Brick(Brick {
        rows: 6.0,
        cols: 3.0,
        mortar: 0.08,
        output: rusty::procgen::recipe::BrickOutput::Mask,
        seed: 0,
    });
    let curvature = OpKind::Curvature(Curvature {
        radius: 0.03,
        strength: 2.0,
    });
    for op in [OpKind::Cavity(Cavity { radius: 0.05 }), curvature] {
        let label = format!("{op:?}");
        let img = bake(vec![node("h", height.clone(), &[]), node("m", op, &["h"])]);
        assert_seamless(&label, &img);
    }
}
