//! src/procgen/bake_set.rs — bake a multi-output recipe to a whole map set (#403).
//!
//! A PBR material needs albedo, normal and roughness derived from the **same**
//! pattern, so the mortar lines match across maps. A recipe's `outputs` names one
//! node per slot; [`bake_set`] evaluates them all in one pass of the runner (shared
//! upstream nodes run once) and writes each with its slot's encoding to
//! `<prefix>_<slot>.png`. Each map is byte-identical to baking that node singly.

use std::collections::BTreeMap;

use super::bake::{bake_to_png, Slot};
use super::recipe::TextureRecipe;
use super::runner::evaluate_many;

/// Evaluate every entry of `recipe.outputs` once and bake it for its slot to
/// `<prefix>_<slot>.png`. Returns canonical slot name → written path.
///
/// Everything is checked before any file is written: an empty `outputs`, an unknown
/// slot, two keys naming the same slot (`albedo` and `base_color`), or an unknown
/// node id is an error and the disk is untouched.
pub fn bake_set(recipe: &TextureRecipe, prefix: &str) -> Result<BTreeMap<String, String>, String> {
    if recipe.outputs.is_empty() {
        return Err(
            "recipe has no `outputs`; give it e.g. outputs = { base_color = \"<node id>\" } \
             (or bake one map with Texture.Bake)"
                .into(),
        );
    }
    let mut slots: BTreeMap<&str, (Slot, &str)> = BTreeMap::new();
    for (key, node) in &recipe.outputs {
        let slot = Slot::parse(key)?;
        if let Some((_, other)) = slots.insert(slot.name(), (slot, node.as_str())) {
            return Err(format!(
                "outputs name the {} slot twice (node {other:?} and {node:?})",
                slot.name()
            ));
        }
    }
    let targets: Vec<&str> = slots.values().map(|(_, node)| *node).collect();
    let images = evaluate_many(recipe, &targets).map_err(|e| e.to_string())?;

    let mut written = BTreeMap::new();
    for ((name, (slot, _)), img) in slots.iter().zip(&images) {
        let path = bake_to_png(img, *slot, &format!("{prefix}_{name}.png"))?;
        written.insert(name.to_string(), path);
    }
    Ok(written)
}
