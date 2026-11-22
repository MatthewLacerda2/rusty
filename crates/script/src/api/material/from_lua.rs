//! Marshaling goes Lua table or JSON string -> `serde_json::Value` -> `MaterialAsset`; the
//! validation-bearing fields are split off into [`Validated`] so the caller applies them
//! through the shared `authoring::material` ops.

use mlua::Value;

use crate::api::lua_json::{decode, recipe_json};
use rusty_components::MaterialAsset;
use rusty_edit::authoring::material::{material_fields, SurfaceField, SurfaceKind, SurfaceValue};
use rusty_procgen::TextureRecipe;

#[derive(Default)]
pub struct Validated {
    pub render_mode: Option<String>,
    pub alpha: Option<f32>,
    pub alpha_cutoff: Option<f32>,
    pub shader: Option<String>,
    pub shader_textures: Vec<(String, String)>,
    pub surface: Vec<(&'static SurfaceField, SurfaceValue)>,
}

fn maps_recipe(value: Option<serde_json::Value>) -> Result<Option<TextureRecipe>, String> {
    value
        .map(decode::<TextureRecipe>)
        .transpose()
        .map_err(|e| format!("maps is a texture recipe (see Texture.BakeSet): {e}"))
}

pub fn asset_from_lua(value: &Value) -> Result<(MaterialAsset, Validated), String> {
    asset_from_json_value(recipe_json(value)?)
}

/// The validated keys are removed before `decode` so serde never applies its own handling of
/// them.
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
        for field in material_fields() {
            if let Some(v) = obj.remove(field.key) {
                validated.surface.push((field, surface_value(field, &v)?));
            }
        }
        maps = maps_recipe(obj.remove("maps"))?;
    }
    refuse_unknown_keys(&value)?;
    let mut asset: MaterialAsset = decode(value)?;
    asset.maps_recipe = maps;
    asset.decal = asset.decal.clamped();
    rusty_edit::authoring::material::clamp_detail(&mut asset);
    Ok((asset, validated))
}

fn surface_value(field: &SurfaceField, v: &serde_json::Value) -> Result<SurfaceValue, String> {
    let value = match &field.kind {
        SurfaceKind::Choice(_) => v.as_str().map(|s| SurfaceValue::Text(s.to_owned())),
        SurfaceKind::Toggle => v.as_bool().map(SurfaceValue::Bool),
        SurfaceKind::Number(_) => v.as_f64().map(|n| SurfaceValue::Number(n as f32)),
        SurfaceKind::Color(_) => serde_json::from_value::<[f32; 3]>(v.clone())
            .ok()
            .map(SurfaceValue::Color),
        SurfaceKind::Vec2(_) => pair(v).map(SurfaceValue::Vec2),
    };
    value
        .ok_or_else(|| field.wrong_kind())
        .and_then(|v| field.check(v))
}

fn pair(v: &serde_json::Value) -> Option<[f32; 2]> {
    let lane = |i: usize, name: &str| v.get(i).or_else(|| v.get(name))?.as_f64();
    Some([lane(0, "x")? as f32, lane(1, "y")? as f32])
}

fn shader_textures(value: Option<serde_json::Value>) -> Result<Vec<(String, String)>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let map: std::collections::BTreeMap<String, String> = serde_json::from_value(value)
        .map_err(|e| format!("shader_textures maps slot names to texture paths: {e}"))?;
    for slot in map.keys() {
        rusty_shadergen::textures::slot(slot)?;
    }
    Ok(map.into_iter().collect())
}

/// Unknown keys are refused here, not by `MaterialAsset`'s serde: it is also the scene-file
/// shape, which stays lenient.
fn refuse_unknown_keys(value: &serde_json::Value) -> Result<(), String> {
    // Valid keys a default asset never serializes (each skips at its default), so `known`
    // lacks them.
    const UNWRITTEN: [&str; 5] = [
        "shader_textures",
        "maps",
        "receive_decals",
        "decal",
        "detail",
    ];
    let known = serde_json::to_value(MaterialAsset::default()).map_err(|e| e.to_string())?;
    let (Some(given), Some(known)) = (value.as_object(), known.as_object()) else {
        return Ok(());
    };
    let is_known = |k: &String| known.contains_key(k) || UNWRITTEN.contains(&k.as_str());
    match given.keys().find(|k| !is_known(k)) {
        Some(key) => {
            let mut valid: Vec<&str> = known.keys().map(String::as_str).collect();
            valid.extend(UNWRITTEN);
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
        assert_eq!(asset.base_color_map, Some("albedo.png".into()));
        assert_eq!(asset.metallic, 0.7);
        assert_eq!(asset.roughness, 0.2);
        assert_eq!(asset.metallic_map, Some("mr.png".into()));
        assert_eq!(asset.roughness_map, Some("mr.png".into()));
        assert_eq!(asset.normal_map, Some("n.png".into()));
        assert_eq!(asset.emissive, [1.0, 0.0, 0.0]);
        assert_eq!(asset.emissive_map, Some("e.png".into()));
    }

    #[test]
    fn validated_fields_are_lifted_out_not_serde_decoded() {
        let lua = Lua::new();
        let t: Value = lua
            .load(r#"return { render_mode = "Cutout", alpha = 0.25, alpha_cutoff = 0.7 }"#)
            .eval()
            .unwrap();
        let (asset, v) = asset_from_lua(&t).expect("recipe parses");
        assert_eq!(asset.alpha, MaterialAsset::default().alpha);
        assert_eq!(v.render_mode.as_deref(), Some("Cutout"));
        assert_eq!(v.alpha, Some(0.25));
        assert_eq!(v.alpha_cutoff, Some(0.7));
    }

    #[test]
    fn unknown_render_mode_string_is_not_a_parse_error() {
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

    #[test]
    fn the_decal_block_and_receive_flag_decode_and_clamp() {
        let lua = Lua::new();
        let recipe = r#"return { receive_decals = false,
            decal = { normal = 0, roughness = 3, angle_fade = 120 } }"#;
        let (asset, _) = asset_from_lua(&lua.load(recipe).eval().unwrap()).unwrap();
        assert!(!asset.receive_decals);
        assert_eq!(asset.decal.normal, 0.0);
        assert_eq!(asset.decal.roughness, 1.0, "weights clamp to [0, 1]");
        assert_eq!(asset.decal.angle_fade, 90.0, "the angle clamps to 90°");
        assert_eq!(
            asset.decal.albedo, 1.0,
            "an omitted weight keeps its default"
        );
        let typo: Value = lua
            .load("return { decal = { rougness = 1 } }")
            .eval()
            .unwrap();
        let err = asset_from_lua(&typo).err().expect("typo refused");
        assert!(err.contains("rougness"), "{err}");
    }
}
