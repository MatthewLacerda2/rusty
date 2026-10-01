//! `Material.SetShaderParam` / `GetShaderParam` (#399): values land on the material
//! under their canonical name, read back (defaults included), and a param the shader
//! does not expose at runtime is an error naming it.

use std::cell::RefCell;

use mlua::Lua;

use crate::scene::authoring::material as mat_ops;
use crate::scene::Scene;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// Bake a toon + hit-flash surface shader under a name unique to this test run,
/// into the workspace the API resolves from; removed on drop.
struct Baked(String);

impl Baked {
    fn new(tag: &str) -> Self {
        let name = format!("test_params_{tag}_{}", std::process::id());
        let blocks = ["toon_ramp", "hit_flash"].map(|id| BlockSel {
            id: id.into(),
            params: Default::default(),
        });
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: name.clone(),
            blocks: blocks.to_vec(),
        };
        bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).unwrap();
        Self(name)
    }
}

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A scene with one entity whose material names `shader`; returns (scene, id).
fn scene_with(shader: &str) -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("E".into());
    let key = mat_ops::ensure_material_key(&mut scene, id).unwrap();
    mat_ops::set_shader(&mut scene.materials, &key, shader.to_string());
    (RefCell::new(scene), id)
}

/// Run `script` against the `Material` namespace, returning its error if any.
fn run(scene: &RefCell<Scene>, script: &str) -> Result<(), String> {
    let lua = Lua::new();
    lua.scope(|s| {
        super::super::register(&lua, s, scene).unwrap();
        lua.load(script).exec()
    })
    .map_err(|e| e.to_string())
}

#[test]
fn set_stores_by_canonical_name_and_get_reads_it_back() {
    let shader = Baked::new("set");
    let (scene, id) = scene_with(&shader.0);
    run(
        &scene,
        &format!(
            r#"assert(Material.GetShaderParam({id}, "hit_flash.amount") == 0)
               Material.SetShaderParam({id}, "hit_flash.1.amount", 0.75)
               Material.SetShaderParam({id}, "hit_flash.color", {{1, 0, 0}})
               assert(Material.GetShaderParam({id}, "hit_flash.amount") == 0.75)
               assert(Material.GetShaderParam({id}, "hit_flash.color")[2] == 0)"#
        ),
    )
    .unwrap();
    let scene = scene.borrow();
    let key = &scene.world.material(id).unwrap().material;
    let stored = &scene.materials[key].shader_params;
    assert_eq!(stored["hit_flash.amount"], [0.75]);
    assert_eq!(stored["hit_flash.color"], [1.0, 0.0, 0.0]);
    assert_eq!(stored.len(), 2, "one value per param, whatever name set it");
}

#[test]
fn a_baked_or_unknown_param_or_bad_value_is_an_error_naming_it() {
    let shader = Baked::new("strict");
    let (scene, id) = scene_with(&shader.0);
    for (call, want) in [
        (r#""toon_ramp.steps", 2"#, "is baked"),
        (r#""hit_flash.amont", 1"#, "hit_flash.amount"),
        (r#""hit_flash.color", {1, 2}"#, "takes 3"),
        (r#""hit_flash.amount", "x""#, "number"),
    ] {
        let err = run(&scene, &format!("Material.SetShaderParam({id}, {call})")).unwrap_err();
        assert!(err.contains(want), "{call}: {err}");
    }
}

#[test]
fn a_material_without_a_baked_shader_is_an_error() {
    for shader in ["", "never_baked_399"] {
        let (scene, id) = scene_with(shader);
        let err = run(
            &scene,
            &format!(r#"Material.SetShaderParam({id}, "a.b", 1)"#),
        )
        .unwrap_err();
        assert!(err.contains("shader"), "{shader:?}: {err}");
    }
}
