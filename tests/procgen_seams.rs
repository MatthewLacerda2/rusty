//! #392's guard: every generator (and `mapping`) bakes **seamless** — the jump across
//! the wrap (last column → first column, last row → first row) is no bigger than the
//! biggest jump between two neighbouring pixels inside the tile. Params deliberately
//! include fractional counts, the values that used to open a seam.

use rusty::procgen::recipe::{GradientKind, Node, NoiseKind, OpKind, TextureRecipe};
use rusty::procgen::recipe::{NoiseKind::*, VoronoiOutput::*, WaveKind};
use rusty::procgen::{evaluate, Image};

const RES: u32 = 64;

pub fn node(id: &str, op: OpKind, inputs: &[&str]) -> Node {
    let inputs = inputs.iter().map(|s| s.to_string()).collect();
    Node {
        id: id.into(),
        op,
        inputs,
    }
}

pub fn bake(nodes: Vec<Node>) -> Image {
    let recipe = TextureRecipe {
        resolution: RES,
        seed: 7,
        nodes,
        output: None,
        outputs: Default::default(),
    };
    evaluate(&recipe).expect("recipe evaluates")
}

/// Biggest per-channel RGB difference between two pixels.
fn jump(a: [f32; 4], b: [f32; 4]) -> f32 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max)
}

/// (seam jump, biggest interior neighbour jump) along x (`horizontal`) or y.
fn seam_and_interior(img: &Image, horizontal: bool) -> (f32, f32) {
    let n = RES as i64;
    let at = |i: i64, j: i64| {
        if horizontal {
            img.get_wrapped(i, j)
        } else {
            img.get_wrapped(j, i)
        }
    };
    let (mut seam, mut interior) = (0.0f32, 0.0f32);
    for j in 0..n {
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

fn generators() -> Vec<(String, OpKind)> {
    let mut ops = Vec::new();
    for s in [1.0, 3.0, 4.5, 5.5, 7.3] {
        for kind in [Perlin, Fbm, Ridged, Turbulence] {
            ops.push((
                format!("noise {kind:?} {s}"),
                OpKind::Noise {
                    kind,
                    scale: s,
                    octaves: 3,
                    lacunarity: s,
                    gain: 0.5,
                },
            ));
        }
        for output in [Distance, F2, Edges] {
            let randomness = s / 8.0;
            let op = OpKind::Voronoi {
                scale: s,
                output,
                randomness,
            };
            ops.push((format!("voronoi {output:?} {s}"), op));
        }
        for kind in [WaveKind::Bands, WaveKind::Rings] {
            ops.push((
                format!("wave {kind:?} {s}"),
                OpKind::Wave { kind, frequency: s },
            ));
        }
    }
    for kind in [GradientKind::Radial, GradientKind::LinearTiling] {
        ops.push((format!("gradient {kind:?}"), OpKind::Gradient { kind }));
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
        let src = OpKind::Noise {
            kind: NoiseKind::Perlin,
            scale: 3.0,
            octaves: 1,
            lacunarity: 2.0,
            gain: 0.5,
        };
        let map = OpKind::Mapping {
            scale,
            rotation,
            translation: [0.13, 0.4],
            tiling: true,
        };
        let img = bake(vec![node("n", src, &[]), node("m", map, &["n"])]);
        assert_seamless(&format!("mapping {scale:?} {rotation}"), &img);
    }
}
