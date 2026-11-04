//! A stochastic node's own `seed` (#1487): two nodes that differ only by it differ.
//! That every seeded op changes with its seed, and that 0 bakes what no seed does,
//! is checked op by op from the catalog in `procgen::ops`'s `seed_tests`.

use rusty::procgen::{evaluate, TextureRecipe};

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
