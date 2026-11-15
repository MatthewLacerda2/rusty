//! `tile`'s bakes at the default `aspect` and `offset`, pinned (#2187): the grid
//! and the rigid turn share the stamp path with every recipe written before them,
//! so an edit to it that moves an old recipe's bytes fails here. The hashes were
//! taken on the code before `aspect` and `offset` existed. The recipes keep
//! `rotation_jitter` at 0 and the sources free of trig, so no `sin` or `cos` (whose
//! last bit is the platform libm's) reaches the bytes: the hashes hold on every
//! platform.

use serde_json::{json, Value};

use crate::evaluate;
use crate::ops::test_graph::{bits, node};
use crate::recipe::TextureRecipe;

/// FNV-1a over an image's bit patterns.
fn fnv(words: &[u32]) -> u64 {
    words
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
}

/// `op` baked at 48 × 32 on a soft disc stamp, a checker mask and a ramp size
/// map (as many as `wired`), recipe seed 3, hashed.
fn hash(op: Value, wired: usize) -> u64 {
    let sources = [
        json!({ "op": "shape", "kind": "circle", "size": [0.7, 0.7], "softness": 0.3 }),
        json!({ "op": "checker", "tiles": 2, "color_a": [1, 1, 1, 1], "color_b": [0, 0, 0, 1] }),
        json!({ "op": "gradient", "kind": "linear" }),
    ];
    let ids = ["s", "m", "z"];
    let mut nodes: Vec<_> = (0..wired)
        .map(|i| node(ids[i], sources[i].clone(), &[]))
        .collect();
    nodes.push(node("out", op, &ids[..wired]));
    let recipe = TextureRecipe::new(0, nodes)
        .with_size([48, 32])
        .with_seed(3)
        .with_output("out");
    fnv(&bits(&evaluate(&recipe).unwrap()))
}

#[test]
fn old_recipes_bake_the_bytes_they_always_did() {
    let recipes = [
        (json!({ "op": "tile", "count": 4 }), 1),
        (
            json!({ "op": "tile", "count": 5, "jitter": 0.7, "scale_jitter": 0.4, "seed": 9 }),
            1,
        ),
        (
            json!({ "op": "tile", "count": 3, "output": "random", "value_jitter": 0.5,
                 "probability": 0.6, "blend": "add" }),
            1,
        ),
        (
            json!({ "op": "tile", "count": 6, "jitter": 1, "output": "index", "blend": "over" }),
            1,
        ),
        (
            json!({ "op": "tile", "count": 4, "jitter": 0.4, "scale_jitter": 0.3 }),
            3,
        ),
    ];
    let got: Vec<u64> = recipes
        .into_iter()
        .map(|(op, wired)| hash(op, wired))
        .collect();
    assert_eq!(got, PINNED);
}

/// Taken by running this test, with zeros here, on `main` before `aspect` and `offset`.
const PINNED: [u64; 5] = [
    0xe840_5eea_0f28_c425,
    0xf9ab_b73c_2aea_d4f7,
    0x009d_1aff_8566_41a9,
    0x8b1a_4103_50c0_a9dc,
    0xc667_e117_cc63_dba9,
];
