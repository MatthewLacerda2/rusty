//! src/api/material/from_lua.rs — marshal a Lua material recipe into a `MaterialAsset`.
//!
//! The recipe table mirrors the `MaterialAsset` serde document one-to-one, so a material
//! authored in Lua and one loaded from `.json` describe the same asset:
//!
//! ```lua
//! {
//!   base_color = {0.5, 0.25, 0.125}, base_color_map = "albedo.png",
//!   metallic = 0.0, metallic_map = "mr.png",
//!   roughness = 0.8, roughness_map = "mr.png",
//!   normal_map = "n.png",
//!   emissive = {0.0, 0.0, 0.0}, emissive_map = "e.png",
//!   render_mode = "Cutout", alpha = 1.0, alpha_cutoff = 0.5,
//!   shader = "enemy_toon", shader_textures = { mask = "noise.png" },
//!   maps = { resolution = 512, nodes = { ... }, outputs = { base_color = "albedo" } },
//! }
//! ```
//!
//! `maps` (#409) is a multi-output texture recipe; it is parsed here and rides on
//! the asset (`maps_recipe`), and the caller bakes it.
//!
//! Marshaling goes Lua table or JSON string → `serde_json::Value` (via the shared
//! [`crate::api::lua_json`] converter) → `MaterialAsset`, so the factor/map decoding
//! lives ONCE in `MaterialAsset`'s serde derive. The THREE validation-bearing fields —
//! `render_mode` (string parse), `alpha` and `alpha_cutoff` (`[0,1]` clamps) — are split
//! off into [`Validated`] so the caller can apply them through the shared
//! `authoring::material::set_*` ops instead of taking serde's raw values. That keeps the
//! validation single-sourced: an unknown `render_mode` degrades to `Opaque` and an
//! out-of-range alpha clamps, exactly as the per-entity setters do.

use mlua::Value;

use crate::api::lua_json::recipe_json;
use crate::components::MaterialAsset;
use crate::procgen::TextureRecipe;

/// The validation-bearing fields lifted out of the recipe so the caller applies them
/// through the shared `set_*` ops. `None` means "not authored" — leave the asset's
/// default in place (which for a fresh asset is the serde default).
#[derive(Default)]
pub struct Validated {
    /// The raw `render_mode` recipe string (e.g. `"Cutout"`), parsed by the shared
    /// `parse_render_mode` so an unknown name degrades to `Opaque`.
    pub render_mode: Option<String>,
    /// The raw `alpha`, clamped to `[0,1]` by the shared `set_alpha` op.
    pub alpha: Option<f32>,
    /// The raw `alpha_cutoff`, clamped to `[0,1]` by the shared `set_alpha_cutoff` op.
    pub alpha_cutoff: Option<f32>,
    /// The raw `shader` name (#396), applied by the shared `set_shader` op so `""`
    /// clears it exactly as `Material.SetShader` does.
    pub shader: Option<String>,
    /// The raw `shader_textures` slot → path map (#400), applied by the shared
    /// `set_shader_texture` op so an unknown slot is refused by name.
    pub shader_textures: Vec<(String, String)>,
}

/// Parse a material recipe's `maps` value (#409) — a multi-output `TextureRecipe`
/// document — so a bad recipe is refused, naming the key, before anything is
/// defined or baked. An absent `maps` is none.
fn maps_recipe(value: Option<serde_json::Value>) -> Result<Option<TextureRecipe>, String> {
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("maps is a texture recipe (see Texture.BakeSet): {e}"))
}

/// Parse a material recipe — a Lua table or its JSON string (#410) — into a base
/// [`MaterialAsset`] (factors + map slots) plus the [`Validated`] fields the caller
/// must apply through the shared ops. Errors carry a message the REPL/script
/// surfaces verbatim.
pub fn asset_from_lua(value: &Value) -> Result<(MaterialAsset, Validated), String> {
    asset_from_json_value(recipe_json(value)?)
}

/// Split a recipe JSON object into the serde-decoded base asset and the validated
/// fields. The three validated keys are REMOVED before `from_value` so serde never
/// applies its own (unclamped / error-on-unknown) handling of them.
fn asset_from_json_value(
    mut value: serde_json::Value,
) -> Result<(MaterialAsset, Validated), String> {
    let mut validated = Validated::default();
    let mut maps = None;
    if let Some(obj) = value.as_object_mut() {
        validated.render_mode = obj
            .remove("render_mode")
            .and_then(|v| v.as_str().map(str::to_string));
        validated.alpha = obj
            .remove("alpha")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        validated.alpha_cutoff = obj
            .remove("alpha_cutoff")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        validated.shader = obj
            .remove("shader")
            .and_then(|v| v.as_str().map(str::to_string));
        validated.shader_textures = shader_textures(obj.remove("shader_textures"))?;
        maps = maps_recipe(obj.remove("maps"))?;
    }
    refuse_unknown_keys(&value)?;
    let mut asset: MaterialAsset = serde_json::from_value(value).map_err(|e| e.to_string())?;
    asset.maps_recipe = maps;
    Ok((asset, validated))
}

/// The recipe's `shader_textures` table (`{ mask = "noise.png" }`) as slot → path
/// pairs; an absent table is none. An unknown slot is refused here, before the
/// asset is defined, so a bad recipe leaves the library untouched.
fn shader_textures(value: Option<serde_json::Value>) -> Result<Vec<(String, String)>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let map: std::collections::BTreeMap<String, String> = serde_json::from_value(value)
        .map_err(|e| format!("shader_textures maps slot names to texture paths: {e}"))?;
    for slot in map.keys() {
        crate::shadergen::textures::slot(slot)?;
    }
    Ok(map.into_iter().collect())
}

/// Refuse a recipe key `MaterialAsset` has no field for (#395), so a typo like
/// `metalic_map` is an error rather than a silently missing map. The asset's own
/// serialized form is the list of valid keys. (`MaterialAsset` itself stays
/// lenient: it is also the scene-file shape, which must tolerate old keys.)
fn refuse_unknown_keys(value: &serde_json::Value) -> Result<(), String> {
    let known = serde_json::to_value(MaterialAsset::default()).map_err(|e| e.to_string())?;
    let (Some(given), Some(known)) = (value.as_object(), known.as_object()) else {
        return Ok(());
    };
    match given.keys().find(|k| !known.contains_key(*k)) {
        Some(key) => {
            // The lifted keys are valid too, though an empty asset never writes them.
            let mut valid: Vec<&str> = known.keys().map(String::as_str).collect();
            valid.extend(["shader_textures", "maps"]);
            Err(format!(
                "unknown material key {key:?}; expected one of {}",
                valid.join(", ")
            ))
        }
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn factors_and_maps_decode_through_serde() {
        let lua = Lua::new();
        let t: Value = lua
            .load(
                r#"return {
                    base_color = {0.5, 0.25, 0.125}, base_color_map = "albedo.png",
                    metallic = 0.7, roughness = 0.2,
                    metallic_map = "mr.png", roughness_map = "mr.png",
                    normal_map = "n.png", emissive = {1.0, 0.0, 0.0},
                    emissive_map = "e.png"
                }"#,
            )
            .eval()
            .unwrap();
        let (asset, _) = asset_from_lua(&t).expect("recipe parses");
        assert_eq!(asset.base_color, [0.5, 0.25, 0.125]);
        assert_eq!(asset.base_color_map.as_deref(), Some("albedo.png"));
        assert_eq!(asset.metallic, 0.7);
        assert_eq!(asset.roughness, 0.2);
        assert_eq!(asset.metallic_map.as_deref(), Some("mr.png"));
        assert_eq!(asset.roughness_map.as_deref(), Some("mr.png"));
        assert_eq!(asset.normal_map.as_deref(), Some("n.png"));
        assert_eq!(asset.emissive, [1.0, 0.0, 0.0]);
        assert_eq!(asset.emissive_map.as_deref(), Some("e.png"));
    }

    #[test]
    fn validated_fields_are_lifted_out_not_serde_decoded() {
        let lua = Lua::new();
        let t: Value = lua
            .load(r#"return { render_mode = "Cutout", alpha = 0.25, alpha_cutoff = 0.7 }"#)
            .eval()
            .unwrap();
        let (asset, v) = asset_from_lua(&t).expect("recipe parses");
        // The base asset keeps the serde defaults for the lifted fields...
        assert_eq!(asset.alpha, MaterialAsset::default().alpha);
        // ...and the raw values ride in `Validated` for the caller's shared ops.
        assert_eq!(v.render_mode.as_deref(), Some("Cutout"));
        assert_eq!(v.alpha, Some(0.25));
        assert_eq!(v.alpha_cutoff, Some(0.7));
    }

    #[test]
    fn unknown_render_mode_string_is_not_a_parse_error() {
        // An unknown render_mode is lifted out as a raw string (degraded later by the
        // shared parser), so it never trips serde's enum decoding.
        let lua = Lua::new();
        let t: Value = lua
            .load(r#"return { render_mode = "glass", metallic = 0.1 }"#)
            .eval()
            .unwrap();
        let (asset, v) = asset_from_lua(&t).expect("unknown mode does not error");
        assert_eq!(asset.metallic, 0.1);
        assert_eq!(v.render_mode.as_deref(), Some("glass"));
    }

    #[test]
    fn empty_recipe_yields_a_default_asset() {
        let lua = Lua::new();
        let t: Value = lua.load(r#"return {}"#).eval().unwrap();
        let (asset, v) = asset_from_lua(&t).expect("empty recipe parses");
        assert_eq!(asset.base_color, MaterialAsset::default().base_color);
        assert!(v.render_mode.is_none() && v.alpha.is_none());
    }

    #[test]
    fn an_unknown_key_is_refused_by_name() {
        let lua = Lua::new();
        let json: Value = lua
            .load(r#"return '{ "metalic_map": "mr.png" }'"#)
            .eval()
            .unwrap();
        let err = asset_from_lua(&json).err().expect("typo refused");
        assert!(
            err.contains("\"metalic_map\"") && err.contains("metallic_map"),
            "{err}"
        );
    }
}
