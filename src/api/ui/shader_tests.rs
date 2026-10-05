//! `UI.SetShader` / `GetShader` / `SetShaderParam` / `GetShaderParam` (#427): the
//! shader lands on every graphic an entity has, params are stored by canonical name
//! and checked against the baked layout, and the errors say what to do.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use crate::components::{ImageComponent, TextComponent};
use crate::scene::{Camera, Scene};
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

/// A dissolve ui shader baked under a name unique to this run; removed on drop.
struct Baked(String);

impl Baked {
    fn new() -> Self {
        let name = format!("test_ui_api_{}", std::process::id());
        let recipe = ShaderRecipe {
            pass: PassKind::Ui,
            name: name.clone(),
            blocks: vec![BlockSel {
                id: "dissolve".into(),
                params: Default::default(),
            }],
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).unwrap();
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

/// Run `script` against the `UI` namespace, returning its error if any.
fn run(scene: &RefCell<Scene>, script: &str) -> Result<(), String> {
    let screen = RefCell::new(crate::ui::ScreenSize::default());
    let video = RefCell::new(crate::core::video::VideoSettings::default());
    let camera = RefCell::new(Camera::new(Vec3::ZERO, -90.0, 0.0));
    let lua = Lua::new();
    lua.scope(|s| {
        super::super::register(&lua, s, scene, (&screen, &video), &camera).unwrap();
        lua.load(script).exec()
    })
    .map_err(|e| e.to_string())
}

/// A scene with a labelled image (Image + Text) and a bare entity: `(scene, label, bare)`.
fn scene() -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    let label = scene.add_entity("Label".into());
    scene
        .world
        .set_image(label, Some(ImageComponent::default()));
    scene.world.set_text(label, Some(TextComponent::default()));
    let bare = scene.add_entity("Bare".into());
    (RefCell::new(scene), label, bare)
}

#[test]
fn set_shader_names_it_on_every_graphic_and_nil_clears_it() {
    let (scene, label, bare) = scene();
    run(&scene, &format!("UI.SetShader({label}, 'glitchy')")).unwrap();
    {
        let world = &scene.borrow().world;
        assert_eq!(
            world.image(label).unwrap().shader.as_ref().unwrap().name,
            "glitchy"
        );
        assert_eq!(
            world.text(label).unwrap().shader.as_ref().unwrap().name,
            "glitchy"
        );
    }
    run(
        &scene,
        &format!("assert(UI.GetShader({label}) == 'glitchy')"),
    )
    .unwrap();
    run(
        &scene,
        &format!("UI.SetShader({label}, nil) assert(UI.GetShader({label}) == nil)"),
    )
    .unwrap();
    let e = run(&scene, &format!("UI.SetShader({bare}, 'glitchy')")).unwrap_err();
    assert!(e.contains("no Image, Shape or Text"), "{e}");
}

#[test]
fn params_round_trip_by_canonical_name_and_errors_list_the_runtime_ones() {
    let baked = Baked::new();
    let (scene, label, _) = scene();
    let e = run(
        &scene,
        &format!("UI.SetShaderParam({label}, 'dissolve.amount', 1)"),
    )
    .unwrap_err();
    assert!(e.contains("UI.SetShader"), "set a shader first: {e}");
    run(
        &scene,
        &format!(
            "UI.SetShader({label}, '{0}')
             assert(UI.GetShaderParam({label}, 'dissolve.amount') == 0)
             UI.SetShaderParam({label}, 'dissolve.0.amount', 0.5)
             assert(UI.GetShaderParam({label}, 'dissolve.amount') == 0.5)",
            baked.0
        ),
    )
    .unwrap();
    let stored = scene
        .borrow()
        .world
        .text(label)
        .unwrap()
        .shader
        .clone()
        .unwrap();
    assert_eq!(
        stored.params["dissolve.amount"],
        vec![0.5],
        "both graphics hold it"
    );
    let e = run(
        &scene,
        &format!("UI.SetShaderParam({label}, 'dissolve.scale', 3)"),
    )
    .unwrap_err();
    assert!(
        e.contains("is baked") && e.contains("dissolve.amount"),
        "{e}"
    );
}
