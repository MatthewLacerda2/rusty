//! A stochastic node's own `seed` (#1487): two nodes that differ only by it differ,
//! and the default 0 bakes the same bytes as a recipe written without it.

use rusty::procgen::recipe::OPS;
use rusty::procgen::{evaluate, TextureRecipe};

/// One node of every stochastic op, each written without a `seed`.
const UNSEEDED: &str = r#"{
  "resolution": 48, "seed": 9,
  "nodes": [
    { "id": "n", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 },
    { "id": "v", "op": "voronoi", "scale": 3, "output": "cells" },
    { "id": "b", "op": "brick", "rows": 4, "cols": 2, "output": "random" },
    { "id": "w", "op": "white_noise" },
    { "id": "t", "op": "tile", "count": 3, "jitter": 0.4, "inputs": ["v"] },
    { "id": "nv", "op": "mix", "mode": "add", "factor": 0.5, "inputs": ["n", "t"] },
    { "id": "bw", "op": "mix", "mode": "multiply", "factor": 0.5, "inputs": ["b", "w"] },
    { "id": "out", "op": "mix", "mode": "screen", "inputs": ["nv", "bw"] }
  ]
}"#;

/// FNV-1a over the output pixels' bit patterns.
fn fingerprint(json: &str) -> u64 {
    let recipe: TextureRecipe = serde_json::from_str(json).expect("recipe parses");
    let img = evaluate(&recipe).expect("recipe evaluates");
    let bits = img.pixels().iter().flatten().map(|c| c.to_bits());
    bits.fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[test]
fn default_seed_bakes_the_bytes_it_did_before_the_param() {
    // `seed: 0` on every stochastic op bakes what no `seed` does, and the runner folds
    // 0 into the recipe seed unchanged (`hash::node_seed`'s test), so a recipe written
    // before #1487 bakes as it did. That was also checked across trees by hand; no
    // committed hash holds it, since a bake differs across platforms' libm.
    let mut explicit = UNSEEDED.to_owned();
    let seeded = OPS
        .iter()
        .filter(|i| i.params.iter().any(|p| p.name == "seed"));
    for op in seeded.map(|i| i.op) {
        let tag = format!(r#""op": "{op}""#);
        explicit = explicit.replace(&tag, &format!(r#"{tag}, "seed": 0"#));
    }
    assert_eq!(fingerprint(&explicit), fingerprint(UNSEEDED));
}

#[test]
fn every_stochastic_op_changes_with_its_seed() {
    let seeded = OPS
        .iter()
        .filter(|i| i.params.iter().any(|p| p.name == "seed"));
    for op in seeded.map(|i| i.op) {
        let tag = format!(r#""op": "{op}""#);
        assert!(UNSEEDED.contains(&tag), "add a {op} node to UNSEEDED");
        let reseeded = UNSEEDED.replace(&tag, &format!(r#"{tag}, "seed": 5"#));
        assert_ne!(fingerprint(&reseeded), fingerprint(UNSEEDED), "{op}");
    }
}

#[test]
fn two_noises_differing_only_in_seed_differ() {
    let two = |seed: u32| {
        format!(
            r#"{{ "resolution": 32, "output": "a", "nodes": [
              {{ "id": "a", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 4 }},
              {{ "id": "b", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 4, "seed": {seed} }}
            ] }}"#
        )
    };
    let a = fingerprint(&two(0));
    let b = fingerprint(&two(0).replace(r#""output": "a""#, r#""output": "b""#));
    let c = fingerprint(&two(1).replace(r#""output": "a""#, r#""output": "b""#));
    assert_eq!(a, b, "seed 0 is the recipe seed");
    assert_ne!(a, c, "seed 1 is an independent pattern");
}
