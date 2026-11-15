//! `color_ramp`'s `linear`, `constant` and `ease` bakes, pinned: they share the stop
//! lookup with the spline fill, so an edit to it that moves an old recipe's bytes
//! fails here. The hashes were taken on the code before `cardinal` and `b_spline`
//! existed. The sources are gradients, whose math is arithmetic and `sqrt` (exact in
//! IEEE 754), so no platform libm reaches the bytes: the hashes hold everywhere.

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

/// `color_ramp` with `stops` and `interpolation` baked at 48 × 32 over a `kind`
/// gradient, hashed.
fn hash(kind: &str, interpolation: &str, stops: Value) -> u64 {
    let nodes = vec![
        node("g", json!({ "op": "gradient", "kind": kind }), &[]),
        node(
            "out",
            json!({ "op": "color_ramp", "stops": stops, "interpolation": interpolation }),
            &["g"],
        ),
    ];
    let recipe = TextureRecipe::new(0, nodes)
        .with_size([48, 32])
        .with_output("out");
    fnv(&bits(&evaluate(&recipe).unwrap()))
}

#[test]
fn old_modes_bake_the_bytes_they_always_did() {
    let uneven = json!([
        { "pos": 0.8, "color": [1, 0.9, 0.2, 1] },
        { "pos": 0.15, "color": [0, 0, 0.1, 1] },
        { "pos": 0.4, "color": [0.5, 0.2, 0.7, 0.6] },
    ]);
    let one = json!([{ "pos": 0.5, "color": [0.3, 0.6, 0.9, 1] }]);
    let twins = json!([
        { "pos": 0.5, "color": [0, 0, 0, 1] },
        { "pos": 0.5, "color": [1, 1, 1, 1] },
        { "pos": 0.9, "color": [0.4, 0.4, 0.4, 1] },
    ]);
    let mut got = Vec::new();
    for mode in ["linear", "constant", "ease"] {
        got.push(hash("linear", mode, uneven.clone()));
        got.push(hash("radial", mode, uneven.clone()));
        got.push(hash("linear", mode, one.clone()));
        got.push(hash("radial", mode, twins.clone()));
    }
    assert_eq!(got, PINNED);
}

/// Taken by running this test, with zeros here, on `main` before `cardinal` and
/// `b_spline`.
const PINNED: [u64; 12] = [
    0x27e5_8add_dda6_b1e5,
    0x83ad_4b56_0be8_b55d,
    0x0e67_49d8_5a64_7b25,
    0x771d_8bde_a458_7425,
    0xad2c_252e_1246_dda5,
    0x5777_43f9_1dd4_da15,
    0x0e67_49d8_5a64_7b25,
    0xea7f_85b0_4e3a_3de5,
    0x4957_77f9_31c7_6ce5,
    0x24bd_a627_999c_a555,
    0x0e67_49d8_5a64_7b25,
    0xf5fa_0e23_db0c_609d,
];
