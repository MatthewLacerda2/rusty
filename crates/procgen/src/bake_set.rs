use std::collections::BTreeMap;

use super::bake::{bake_to_png, Slot};
use super::recipe::TextureRecipe;
use super::runner::evaluate_many;
use super::source::Sources;

/// Everything is checked before any file is written: an empty `outputs`, an unknown
/// slot, two keys naming the same slot (`albedo` and `base_color`, or `normal` and
/// `normal16`), or an unknown
/// node id is an error and the disk is untouched.
pub fn bake_set(
    recipe: &TextureRecipe,
    sources: &Sources,
    prefix: &str,
) -> Result<BTreeMap<String, String>, String> {
    let slots = set_slots(recipe)?;
    let targets: Vec<&str> = slots.values().map(|(_, node)| *node).collect();
    let images = evaluate_many(recipe, sources, &targets).map_err(|e| e.to_string())?;

    let mut written = BTreeMap::new();
    for ((name, (slot, _)), img) in slots.iter().zip(&images) {
        let path = bake_to_png(img, *slot, &set_path(prefix, name))?;
        written.insert(name.to_string(), path);
    }
    Ok(written)
}

/// A caller runs this before reading the recipe's image files, so a recipe with no
/// `outputs` is reported as that, not as a file it could not read.
pub fn check_set(recipe: &TextureRecipe) -> Result<(), String> {
    set_slots(recipe).map(|_| ())
}

/// Checked as [`check_set`] does, so a caller can ready the paths' sidecars before the bake.
pub fn set_paths(recipe: &TextureRecipe, prefix: &str) -> Result<Vec<String>, String> {
    let slots = set_slots(recipe)?;
    Ok(slots.keys().map(|name| set_path(prefix, name)).collect())
}

fn set_path(prefix: &str, name: &str) -> String {
    format!("{prefix}_{name}.png")
}

fn set_slots(recipe: &TextureRecipe) -> Result<BTreeMap<&'static str, (Slot, &str)>, String> {
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
        let twin = slots.values().find(|(s, _)| s.kind() == slot.kind());
        if let Some((_, other)) = twin {
            return Err(format!(
                "outputs name the {} slot twice (node {other:?} and {node:?})",
                slot.kind().name()
            ));
        }
        slots.insert(slot.name(), (slot, node.as_str()));
    }
    Ok(slots)
}
