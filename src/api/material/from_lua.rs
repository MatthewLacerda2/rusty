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
//! }
//! ```
//!
//! Marshaling goes Lua table → `serde_json::Value` (via the shared
//! [`crate::api::lua_json`] converter) → `MaterialAsset`, so the factor/map decoding
//! lives ONCE in `MaterialAsset`'s serde derive. The THREE validation-bearing fields —
//! `render_mode` (string parse), `alpha` and `alpha_cutoff` (`[0,1]` clamps) — are split
//! off into [`Validated`] so the caller can apply them through the shared
//! `authoring::material::set_*` ops instead of taking serde's raw values. That keeps the
//! validation single-sourced: an unknown `render_mode` degrades to `Opaque` and an
//! out-of-range alpha clamps, exactly as the per-entity setters do.

use mlua::Table;

use crate::api::lua_json::table_to_json;
use crate::components::MaterialAsset;

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
}

/// Parse a Lua material recipe `table` into a base [`MaterialAsset`] (factors + map
/// slots) plus the [`Validated`] fields the caller must apply through the shared ops.
/// Errors carry a message the REPL/script surfaces verbatim.
pub fn asset_from_table(table: &Table) -> Result<(MaterialAsset, Validated), String> {
    let json = table_to_json(table)?;
    asset_from_json_value(json)
}

/// Same as [`asset_from_table`] but from a JSON string (the `DefineAssetJson` form).
pub fn asset_from_json_str(json: &str) -> Result<(MaterialAsset, Validated), String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    asset_from_json_value(value)
}

/// Split a recipe JSON object into the serde-decoded base asset and the validated
/// fields. The three validated keys are REMOVED before `from_value` so serde never
/// applies its own (unclamped / error-on-unknown) handling of them.
fn asset_from_json_value(
    mut value: serde_json::Value,
) -> Result<(MaterialAsset, Validated), String> {
    let mut validated = Validated::default();
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
    }
    let asset: MaterialAsset = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok((asset, validated))
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn factors_and_maps_decode_through_serde() {
        let lua = Lua::new();
        let t: Table = lua
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
        let (asset, _) = asset_from_table(&t).expect("recipe parses");
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
        let t: Table = lua
            .load(r#"return { render_mode = "Cutout", alpha = 0.25, alpha_cutoff = 0.7 }"#)
            .eval()
            .unwrap();
        let (asset, v) = asset_from_table(&t).expect("recipe parses");
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
        let t: Table = lua
            .load(r#"return { render_mode = "glass", metallic = 0.1 }"#)
            .eval()
            .unwrap();
        let (asset, v) = asset_from_table(&t).expect("unknown mode does not error");
        assert_eq!(asset.metallic, 0.1);
        assert_eq!(v.render_mode.as_deref(), Some("glass"));
    }

    #[test]
    fn empty_recipe_yields_a_default_asset() {
        let lua = Lua::new();
        let t: Table = lua.load(r#"return {}"#).eval().unwrap();
        let (asset, v) = asset_from_table(&t).expect("empty recipe parses");
        assert_eq!(asset.base_color, MaterialAsset::default().base_color);
        assert!(v.render_mode.is_none() && v.alpha.is_none());
    }
}
