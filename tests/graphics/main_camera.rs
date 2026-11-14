//! `Graphics.*` edits the main camera (#1434): the screen stack's base, Unity's
//! `Camera.main`. A render-texture camera created first never takes a write meant for
//! the window, and the window's post-FX reads the volume `Graphics` edits.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, RenderTarget};
use rusty::core::settings::GraphicsSettings;
use rusty::scene::authoring::default_post_processing;
use rusty::scene::camera::main_camera;
use rusty::scene::Scene;
use rusty::script::api::bind;

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
        .set_post_processing(id, Some(default_post_processing()));
    id
}

fn run(scene: &Rc<RefCell<Scene>>, script: &str) {
    let lua = Lua::new();
    let quality = Rc::new(RefCell::new(GraphicsSettings::default()));
    let app = Rc::default();
    crate::lua::attempt(|| {
        rusty::script::api::graphics::register(
            &lua,
            bind(&lua, scene),
            (bind(&lua, &quality), rusty::script::api::ApiCell::default()),
            bind(&lua, &app),
        )
        .unwrap();
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
        "Graphics.SetSaturation(0.25) Graphics.SetFxaaActive(false) Graphics.SetTaaActive(true)",
    );

    let s = scene.borrow();
    let saturation = |id| {
        s.world
            .post_processing(id)
            .unwrap()
            .color_adjustments
            .saturation
    };
    let aa = |id| {
        let cam = s.world.camera(id).unwrap();
        (cam.fxaa_active, cam.taa_active)
    };
    assert_eq!((saturation(main), aa(main)), (0.25, (false, true)));
    assert_eq!(
        (saturation(monitor), aa(monitor)),
        (1.0, (true, false)),
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
         assert(Graphics.GetSaturation() == 1.0 and Graphics.GetFxaaActive())
         assert(not Graphics.GetTaaActive())
         -- Nothing blurs, and the whole motion-blur group says so (#1436).
         assert(not Graphics.GetMotionBlurActive() and Graphics.GetMotionBlurSamples() == 0)
         assert(Graphics.GetMotionBlurIntensity() == 0 and Graphics.GetMotionBlurMaxLength() == 0)",
    );
    assert_eq!(
        scene
            .borrow()
            .world
            .post_processing(monitor)
            .unwrap()
            .color_adjustments
            .saturation,
        1.0
    );
}
