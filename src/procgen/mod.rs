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
//! - `bake_set` — a recipe's named `outputs` → one PNG per slot, one evaluation (#403).
//! - [`ops`] — the op-set grouped by category, plus the per-node dispatcher.
//! - [`bands`] — row bands run in parallel; byte-identical at any band count (#1490).
//! - [`hash`] — dependency-free deterministic value hashing for stochastic ops.
//! - [`image_buf`] — the linear-RGBA buffer the runner works in (wrapping domain).
//! - `source` — the decoded files `image` nodes read, handed in by the caller (#1491).

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod bake;
mod bake_set;
pub mod bands;
pub mod hash;
pub mod image_buf;
pub mod ops;
pub mod recipe;
pub mod runner;
mod source;

pub use bake::Slot;
pub use bake_set::{bake_set, check_set};
pub use bands::with_bands;
pub use image_buf::{Image, Size};
pub use recipe::TextureRecipe;
pub use runner::{evaluate, evaluate_with, validate, RunError};
pub use source::{Source, Sources};

/// Evaluate `recipe` over `sources` and bake it to a `.png` at `path` for the named
/// `slot`. The one-call front door the API surface drives: recipe → buffer → encoded
/// PNG.
pub fn bake_recipe(
    recipe: &TextureRecipe,
    sources: &Sources,
    slot: Slot,
    path: &str,
) -> Result<String, String> {
    let img = evaluate_with(recipe, sources).map_err(|e| e.to_string())?;
    bake::bake_to_png(&img, slot, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procgen::recipe::{Checker, Node, Noise, OpKind};

    /// A simple checker recipe used by several spine-level tests.
    fn checker_recipe(resolution: u32, seed: u64) -> TextureRecipe {
        TextureRecipe::new(
            resolution,
            vec![Node {
                id: "c".into(),
                op: OpKind::Checker(Checker {
                    tiles: 4,
                    color_a: [0.0, 0.0, 0.0, 1.0],
                    color_b: [1.0, 1.0, 1.0, 1.0],
                }),
                inputs: vec![],
            }],
        )
        .with_seed(seed)
    }

    #[test]
    fn same_recipe_and_seed_bakes_identical_bytes() {
        let dir = crate::test_temp::dir();
        let a = dir.join("rusty_procgen_det_a.png");
        let b = dir.join("rusty_procgen_det_b.png");
        let recipe = TextureRecipe::new(
            64,
            vec![Node {
                id: "n".into(),
                op: OpKind::Noise(Noise {
                    kind: recipe::NoiseKind::Fbm,
                    scale: 8.0,
                    octaves: 4,
                    lacunarity: 2.0,
                    gain: 0.5,
                    seed: 0,
                }),
                inputs: vec![],
            }],
        )
        .with_seed(1234);
        bake_recipe(&recipe, &Sources::new(), Slot::Data, a.to_str().unwrap()).unwrap();
        bake_recipe(&recipe, &Sources::new(), Slot::Data, b.to_str().unwrap()).unwrap();
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
}
