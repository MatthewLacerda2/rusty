//! src/api/material/asset.rs — standalone named-material-asset authoring (#271).
//!
//! The material leg of #174: author a reusable `MaterialAsset` into the per-World
//! library under a chosen *name*, decoupled from any entity (the per-entity setters in
//! the parent module mutate the material an entity already references; these define a
//! material as an asset from scratch). An entity then uses it by pointing its
//! `MaterialComponent.material` at that name (the existing reference mechanism). The
//! asset round-trips through `SceneData` like any library material — no special-casing.
//!
//! Verbs:
//! - `Material.DefineAsset(name, recipe)` — build a `MaterialAsset` from a recipe
//!   (factors + map-slot paths + render mode + alpha/cutoff) and write it into
//!   `scene.materials` under `name`, overwriting any existing asset of that name. The
//!   recipe is a Lua table **or** its JSON string (the on-disk form, #410), so a saved
//!   recipe re-defines through the same verb without a round-trip through Lua.
//!   A `maps` texture recipe (#409) is baked first, to
//!   `project/assets/textures/<name>_<slot>.png`, filling the map paths the recipe
//!   left empty; a failed bake defines nothing.
//! - `Material.Rebake(name [, resolution])` — re-bake the stored `maps` recipe,
//!   optionally at a new resolution the recipe then keeps.
//! - `Material.GetAsset(name)` — the named asset's canonical JSON (or `nil` if absent),
//!   so the authored asset is observable through the same surface.
//! - `Material.HasAsset(name)` — whether the library holds an asset under `name`.
//!
//! Single-sourcing: the factors + map slots ride serde, but the THREE validation-bearing
//! fields (`render_mode` parse, `alpha`/`alpha_cutoff` clamps) are applied through the
//! shared `authoring::material::set_*` ops — the SAME ops the per-entity setters and the
//! editor card call — so validation lives once.

use std::cell::RefCell;
use std::path::Path;

use super::from_lua::{asset_from_lua, Validated};
use super::{parse_render_mode, put, Reg};
use crate::components::MaterialAsset;
use crate::scene::authoring::material as mat_ops;
use crate::scene::Scene;

/// Register the standalone asset-authoring verbs onto the `Material` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "DefineAsset",
        scope.create_function(|_, (name, recipe): (String, mlua::Value)| {
            let (asset, validated) = asset_from_lua(&recipe).map_err(mlua::Error::RuntimeError)?;
            define(scene, &name, asset, validated).map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "GetAsset",
        scope.create_function(|_, name: String| {
            let scene = scene.borrow();
            Ok(match scene.materials.get(&name) {
                Some(asset) => Some(
                    serde_json::to_string(asset)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?,
                ),
                None => None,
            })
        }),
    )?;

    put(
        table,
        "Rebake",
        scope.create_function(|_, (name, resolution): (String, Option<u32>)| {
            let mut scene = scene.borrow_mut();
            let dir = Path::new(mat_ops::MAPS_DIR);
            mat_ops::rebake_maps(&mut scene.materials, &name, resolution, dir)
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "HasAsset",
        scope.create_function(|_, name: String| Ok(scene.borrow().materials.contains_key(&name))),
    )
}

/// Define `asset` under `name` in the library, applying the validation-bearing fields
/// through the shared `authoring::material::set_*` ops so their parse/clamp stays
/// single-sourced. We insert the base asset first (so the per-field ops have a target),
/// then route each authored validated field through its shared op.
fn define(
    scene: &RefCell<Scene>,
    name: &str,
    mut asset: MaterialAsset,
    validated: Validated,
) -> Result<(), String> {
    mat_ops::bake_maps(&mut asset, name, Path::new(mat_ops::MAPS_DIR))?;
    let mut scene = scene.borrow_mut();
    let materials = &mut scene.materials;
    mat_ops::define_asset(materials, name, asset);
    if let Some(mode) = validated.render_mode {
        mat_ops::set_render_mode(materials, name, parse_render_mode(&mode));
    }
    if let Some(alpha) = validated.alpha {
        mat_ops::set_alpha(materials, name, alpha);
    }
    if let Some(cutoff) = validated.alpha_cutoff {
        mat_ops::set_alpha_cutoff(materials, name, cutoff);
    }
    if let Some(shader) = validated.shader {
        mat_ops::set_shader(materials, name, shader);
    }
    for (slot, path) in validated.shader_textures {
        mat_ops::set_shader_texture(materials, name, &slot, Some(path))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "maps_tests.rs"]
mod maps_tests;
#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;
    use crate::components::RenderMode;

    /// Register the full `Material` namespace against `scene` and run `script`.
    fn run(scene: &RefCell<Scene>, script: &str) {
        let lua = Lua::new();
        lua.scope(|s| {
            super::super::register(&lua, s, scene).unwrap();
            lua.load(script).exec().unwrap();
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_recipe_shader_and_set_shader_name_the_surface_shader() {
        let scene = RefCell::new(Scene::new());
        let id = scene.borrow_mut().add_entity("E".into());
        run(
            &scene,
            &format!(
                r#"Material.DefineAsset("toon", {{ shader = "enemy_toon" }})
                   Material.DefineAsset("plain", {{ shader = "" }})
                   Material.SetShader({id}, "holo")"#
            ),
        );
        let scene = scene.borrow();
        assert_eq!(
            scene.materials["toon"].shader.as_deref(),
            Some("enemy_toon")
        );
        assert_eq!(
            scene.materials["plain"].shader, None,
            "\"\" is the standard one"
        );
        let key = &scene.world.material(id).unwrap().material;
        assert_eq!(scene.materials[key].shader.as_deref(), Some("holo"));
    }

    #[test]
    fn define_asset_writes_factors_and_maps_into_the_library() {
        let scene = RefCell::new(Scene::new());
        run(
            &scene,
            r#"Material.DefineAsset("brick", {
                base_color = {0.5, 0.25, 0.125},
                metallic = 0.0, roughness = 0.8,
                metallic_map = "brick_mr.png", roughness_map = "brick_mr.png",
                normal_map = "brick_n.png",
            })"#,
        );
        let scene = scene.borrow();
        let m = scene.materials.get("brick").expect("asset defined");
        assert_eq!(m.base_color, [0.5, 0.25, 0.125]);
        assert_eq!(m.roughness, 0.8);
        assert_eq!(m.metallic_map.as_deref(), Some("brick_mr.png"));
        assert_eq!(m.roughness_map.as_deref(), Some("brick_mr.png"));
        assert_eq!(m.normal_map.as_deref(), Some("brick_n.png"));
    }

    #[test]
    fn validated_fields_route_through_the_shared_clamp_and_parse() {
        let scene = RefCell::new(Scene::new());
        run(
            &scene,
            r#"Material.DefineAsset("glass", {
                render_mode = "transparent", alpha = 2.0, alpha_cutoff = -1.0,
            })"#,
        );
        let scene = scene.borrow();
        let m = scene.materials.get("glass").unwrap();
        assert_eq!(m.render_mode, RenderMode::Transparent);
        assert_eq!(m.alpha, 1.0, "clamped by shared set_alpha");
        assert_eq!(m.alpha_cutoff, 0.0, "clamped by shared set_alpha_cutoff");
    }

    #[test]
    fn unknown_render_mode_degrades_to_opaque() {
        let scene = RefCell::new(Scene::new());
        run(
            &scene,
            r#"Material.DefineAsset("typo", { render_mode = "shiny" })"#,
        );
        assert_eq!(
            scene.borrow().materials.get("typo").unwrap().render_mode,
            RenderMode::Opaque
        );
    }

    #[test]
    fn has_and_get_asset_observe_the_library() {
        let scene = RefCell::new(Scene::new());
        run(&scene, r#"Material.DefineAsset("m", { metallic = 0.3 })"#);

        let lua = Lua::new();
        let (has, json): (bool, String) = lua
            .scope(|s| {
                super::super::register(&lua, s, &scene).unwrap();
                Ok(lua
                    .load(r#"return Material.HasAsset("m"), Material.GetAsset("m")"#)
                    .eval()
                    .unwrap())
            })
            .unwrap();
        assert!(has);
        let back: MaterialAsset = serde_json::from_str(&json).expect("GetAsset returns valid JSON");
        assert_eq!(back.metallic, 0.3);

        // An absent name reports false / nil.
        let absent: Option<String> = lua
            .scope(|s| {
                super::super::register(&lua, s, &scene).unwrap();
                Ok(lua
                    .load(r#"return Material.GetAsset("nope")"#)
                    .eval()
                    .unwrap())
            })
            .unwrap();
        assert!(absent.is_none());
    }

    #[test]
    fn define_asset_accepts_a_json_string() {
        let scene = RefCell::new(Scene::new());
        run(
            &scene,
            r#"Material.DefineAsset("steel",
                '{ "metallic": 1.0, "roughness": 0.1, "render_mode": "Opaque" }')"#,
        );
        let scene = scene.borrow();
        let m = scene.materials.get("steel").unwrap();
        assert_eq!(m.metallic, 1.0);
        assert_eq!(m.roughness, 0.1);
        assert_eq!(m.render_mode, RenderMode::Opaque);
    }
}
