//! `Graphics.*` edits the main camera (#1434): the screen stack's base, Unity's
//! `Camera.main`. A render-texture camera created first never takes a write meant for
//! the window, and the window's post-FX reads the volume `Graphics` edits.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, RenderTarget};
use rusty::core::settings::quality::QualityPreset;
use rusty::scene::authoring::default_visual_correction;
use rusty::scene::camera::main_camera;
use rusty::scene::Scene;

/// A camera entity with a default volume, on the screen or into `target`.
fn camera(scene: &mut Scene, name: &str, target: Option<RenderTarget>) -> u32 {
    let id = scene.add_entity(name.to_string());
    let comp = CameraComponent {
        target_texture: target,
        ..CameraComponent::default()
    };
    scene.world.set_camera(id, Some(comp));
    scene
        .world
        .set_visual_correction(id, Some(default_visual_correction()));
    id
}

fn run(scene: &Rc<RefCell<Scene>>, script: &str) {
    let lua = Lua::new();
    let quality = Rc::new(RefCell::new(QualityPreset::Medium));
    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, scene, &quality).unwrap();
        lua.load(script).exec()
    })
    .unwrap();
}

#[test]
fn a_render_texture_camera_created_first_does_not_take_the_screens_writes() {
    let mut scene = Scene::new();
    let monitor = camera(
        &mut scene,
        "Monitor",
        Some(RenderTarget::new("cctv", 64, 64)),
    );
    let main = camera(&mut scene, "Main", None);
    assert_eq!(main_camera(&scene), Some(main));
    let scene = Rc::new(RefCell::new(scene));

    run(
        &scene,
        "Graphics.SetSaturation(0.25) Graphics.SetFxaaActive(false)",
    );

    let s = scene.borrow();
    let saturation = |id| s.world.visual_correction(id).unwrap().saturation;
    let fxaa = |id| s.world.camera(id).unwrap().fxaa_active;
    assert_eq!((saturation(main), fxaa(main)), (0.25, false));
    assert_eq!(
        (saturation(monitor), fxaa(monitor)),
        (1.0, true),
        "the monitor keeps its own"
    );
}

#[test]
fn without_a_screen_camera_getters_are_neutral_and_setters_no_ops() {
    let mut scene = Scene::new();
    let monitor = camera(
        &mut scene,
        "Monitor",
        Some(RenderTarget::new("cctv", 64, 64)),
    );
    let scene = Rc::new(RefCell::new(scene));
    run(
        &scene,
        "Graphics.SetSaturation(0.25)
         assert(Graphics.GetSaturation() == 1.0 and Graphics.GetFxaaActive())",
    );
    assert_eq!(
        scene
            .borrow()
            .world
            .visual_correction(monitor)
            .unwrap()
            .saturation,
        1.0
    );
}
