//! Maps come out seamless by construction (the procedural domain wraps) and byte-identical for a
//! given seed (stochastic ops draw from a pure integer hash, never `rand` or
//! wall-clock).

// Panic-free sim: bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod bake;
mod bake_set;
pub mod bands;
pub mod catalog;
pub mod graph;
pub mod hash;
pub mod image_buf;
pub mod ops;
pub mod recipe;
pub mod runner;
mod source;

pub use bake::Slot;
pub use bake_set::{bake_set, check_set, set_paths};
pub use bands::with_bands;
pub use image_buf::{Image, Size};
pub use recipe::TextureRecipe;
pub use runner::{evaluate, evaluate_with, validate, RunError};
pub use rusty_font::FontHandle;
pub use source::{Source, Sources};

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
    use crate::recipe::{Checker, Node, Noise, OpKind};

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
        let dir = rusty_test_support::temp::dir();
        let a = dir.join("rusty_procgen_det_a.png");
        let b = dir.join("rusty_procgen_det_b.png");
        let recipe = TextureRecipe::new(
            64,
            vec![Node {
                id: "n".into(),
                op: OpKind::Noise(Noise {
                    kind: recipe::NoiseKind::Fbm,
                    scale: 8.0,
                    aspect: [1.0, 1.0],
                    octaves: 4,
                    lacunarity: 2.0,
                    gain: 0.5.into(),
                    distortion: 0.0,
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
        let img = evaluate(&checker_recipe(64, 0)).unwrap();
        for y in 0..64 {
            assert_eq!(img.get_wrapped(0, y), img.get_wrapped(64, y));
            assert_eq!(img.get_wrapped(y, 0), img.get_wrapped(y, 64));
        }
    }
}
