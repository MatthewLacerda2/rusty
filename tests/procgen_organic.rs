//! #406's organic ops — domain warp, ridged / turbulence noise, Voronoi F2 / edges —
//! keep the two procgen promises (seamless, resolution-independent) and bake
//! byte-identically for a seed. The generators' seams are swept in `procgen_seams`.

use rusty::procgen::recipe::{Node, NoiseKind, OpKind, TextureRecipe, VoronoiOutput};
use rusty::procgen::{evaluate, Image};

use crate::procgen_resolution::assert_resolution_independent;
use crate::procgen_seams::{assert_seamless, bake, node};

fn noise(id: &str, kind: NoiseKind, scale: f32) -> Node {
    let op = OpKind::Noise {
        kind,
        scale,
        octaves: 3,
        lacunarity: 2.0,
        gain: 0.5,
    };
    node(id, op, &[])
}

/// Rock: ridged noise warped by a field whose R and G are independent noises.
fn warped(strength: f32) -> Vec<Node> {
    vec![
        noise("a", NoiseKind::Ridged, 4.0),
        noise("dx", NoiseKind::Fbm, 3.0),
        noise("dy", NoiseKind::Fbm, 5.0),
        node("d", OpKind::CombineRgb, &["dx", "dy", "dx"]),
        node("out", OpKind::Warp { strength }, &["a", "d"]),
    ]
}

fn bake_seeded(nodes: &[Node], seed: u64) -> Image {
    let recipe = TextureRecipe::new(64, nodes.to_vec()).with_seed(seed);
    evaluate(&recipe).expect("recipe evaluates")
}

#[test]
fn warp_keeps_tiling_inputs_seamless() {
    for strength in [0.05, 0.3, 1.7] {
        assert_seamless(&format!("warp {strength}"), &bake(warped(strength)));
    }
}

#[test]
fn warp_strength_is_in_tile_units() {
    assert_resolution_independent("warp", &warped(0.2), 0.02);
}

#[test]
fn organic_ops_bake_byte_identically_per_seed() {
    let voronoi = |output| {
        let op = OpKind::Voronoi {
            scale: 6.0,
            output,
            randomness: 0.7,
        };
        vec![node("v", op, &[])]
    };
    let recipes = [
        vec![noise("n", NoiseKind::Ridged, 5.0)],
        vec![noise("n", NoiseKind::Turbulence, 5.0)],
        voronoi(VoronoiOutput::F2),
        voronoi(VoronoiOutput::Edges),
        warped(0.4),
    ];
    for nodes in recipes {
        let label = format!("{:?}", nodes.last().map(|n| &n.op));
        let (a, b) = (bake_seeded(&nodes, 11), bake_seeded(&nodes, 11));
        assert_eq!(a.pixels(), b.pixels(), "{label}: same seed, different bake");
        let other = bake_seeded(&nodes, 12);
        assert_ne!(a.pixels(), other.pixels(), "{label}: seed ignored");
    }
}
