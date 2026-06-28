//! src/procgen — agent-composed procedural texture authoring (#270).
//!
//! The agent composes a curated op-set into a serde **recipe**, an **op-runner**
//! evaluates that recipe to a linear-RGBA buffer, and a **bake** step encodes the
//! buffer to a tiling `.png` the material map slots consume. The maps come out
//! seamless by construction (the procedural domain wraps) and byte-identical for a
//! given seed (stochastic ops draw from a pure integer hash, never `rand` or
//! wall-clock).
//!
//! This is the first of the #174 authoring legs, so it also stands up the **shared
//! spine** — recipe / op-runner / bake — that the material (#271) and shader (#272)
//! legs reuse. It is distinct from [`crate::scene::authoring`], which authors
//! *entities/components*; this authors *textures*.
//!
//! Module layout:
//! - [`recipe`] — the serde DAG (`TextureRecipe`, `Node`, `OpKind`): the source of
//!   truth, round-trips losslessly.
//! - [`runner`] — DAG → [`image_buf::Image`] in dependency order.
//! - [`bake`] — buffer → PNG with per-slot glTF encoding/packing.
//! - [`ops`] — the op-set grouped by category, plus the per-node dispatcher.
//! - [`hash`] — dependency-free deterministic value hashing for stochastic ops.
//! - [`image_buf`] — the linear-RGBA buffer the runner works in (wrapping domain).

pub mod bake;
pub mod hash;
pub mod image_buf;
pub mod ops;
pub mod recipe;
pub mod runner;

pub use bake::Slot;
pub use image_buf::Image;
pub use recipe::TextureRecipe;
pub use runner::{evaluate, RunError};

/// Evaluate `recipe` and bake it to a `.png` at `path` for the named `slot`. The
/// one-call front door the API surface drives: recipe → buffer → encoded PNG.
pub fn bake_recipe(recipe: &TextureRecipe, slot: Slot, path: &str) -> Result<String, String> {
    let img = evaluate(recipe).map_err(|e| e.to_string())?;
    bake::bake_to_png(&img, slot, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procgen::recipe::{Node, OpKind};

    /// A simple checker recipe used by several spine-level tests.
    fn checker_recipe(resolution: u32, seed: u64) -> TextureRecipe {
        TextureRecipe {
            resolution,
            seed,
            nodes: vec![Node {
                id: "c".into(),
                op: OpKind::Checker {
                    tiles: 4,
                    color_a: [0.0, 0.0, 0.0, 1.0],
                    color_b: [1.0, 1.0, 1.0, 1.0],
                },
                inputs: vec![],
            }],
            output: None,
        }
    }

    #[test]
    fn same_recipe_and_seed_bakes_identical_bytes() {
        let dir = std::env::temp_dir();
        let a = dir.join("rusty_procgen_det_a.png");
        let b = dir.join("rusty_procgen_det_b.png");
        let recipe = TextureRecipe {
            resolution: 64,
            seed: 1234,
            nodes: vec![Node {
                id: "n".into(),
                op: OpKind::Noise {
                    kind: recipe::NoiseKind::Fbm,
                    scale: 8.0,
                    octaves: 4,
                },
                inputs: vec![],
            }],
            output: None,
        };
        bake_recipe(&recipe, Slot::Data, a.to_str().unwrap()).unwrap();
        bake_recipe(&recipe, Slot::Data, b.to_str().unwrap()).unwrap();
        let ba = std::fs::read(&a).unwrap();
        let bb = std::fs::read(&b).unwrap();
        assert_eq!(ba, bb, "same recipe+seed must bake byte-identical PNG");
        std::fs::remove_file(a).ok();
        std::fs::remove_file(b).ok();
    }

    #[test]
    fn checker_tiles_exactly_left_to_right_and_top_to_bottom() {
        // A checker with a tile count dividing the resolution wraps exactly: the
        // column past the right edge equals column 0, ditto top/bottom.
        let img = evaluate(&checker_recipe(64, 0)).unwrap();
        for y in 0..64 {
            assert_eq!(img.get_wrapped(0, y), img.get_wrapped(64, y));
            assert_eq!(img.get_wrapped(y, 0), img.get_wrapped(y, 64));
        }
    }

    #[test]
    fn recipe_round_trip_re_bakes_identically() {
        let recipe = checker_recipe(48, 99);
        let json = recipe.to_json().unwrap();
        let back = TextureRecipe::from_json(&json).unwrap();
        assert_eq!(recipe, back);
        let a = evaluate(&recipe).unwrap();
        let b = evaluate(&back).unwrap();
        assert_eq!(a, b);
    }
}
