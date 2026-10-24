//! The per-entity `Camera` lens and stacking functions (#1341): field of view, clip
//! planes, render order and clear flags through the Lua surface — what a script
//! needs to build a viewmodel camera over the world one.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, ClearFlags};
use rusty::scene::Scene;

fn run(scene: &RefCell<Scene>, code: &str) -> mlua::Result<()> {
    let lua = Lua::new();
    lua.scope(|scope| {
        let table = lua.create_table()?;
        rusty::api::camera::component::register(scope, &table, scene).unwrap();
        lua.globals().set("Camera", table)?;
        lua.load(code).exec()
    })
}

fn scene_with_camera() -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Viewmodel".to_string());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    let bare = scene.add_entity("Bare".to_string());
    (RefCell::new(scene), cam, bare)
}

#[test]
fn a_viewmodel_camera_is_scriptable() {
    let (scene, cam, bare) = scene_with_camera();
    let code = format!(
        r#"
        assert(Camera.GetRenderOrder({cam}) == 0)
        assert(Camera.GetClearFlags({cam}) == "Skybox")
        Camera.SetRenderOrder({cam}, 1)
        Camera.SetClearFlags({cam}, "depthonly")
        Camera.SetFieldOfView({cam}, 55)
        Camera.SetClipPlanes({cam}, 0.01, 100)
        assert(Camera.GetRenderOrder({cam}) == 1)
        assert(Camera.GetClearFlags({cam}) == "DepthOnly")
        assert(Camera.GetFieldOfView({cam}) == 55)
        local near, far = Camera.GetClipPlanes({cam})
        assert(math.abs(near - 0.01) < 1e-6 and far == 100, near)
        -- Unity's spelling reads too.
        Camera.SetClearFlags({cam}, "Color")
        assert(Camera.GetClearFlags({cam}) == "SolidColor")
        Camera.SetClearFlags({cam}, "Depth")
        -- No camera: nil getters, no-op setters.
        Camera.SetRenderOrder({bare}, 4)
        Camera.SetClipPlanes({bare}, 1, 2)
        assert(Camera.GetRenderOrder({bare}) == nil)
        assert(Camera.GetClearFlags({bare}) == nil)
        assert(Camera.GetFieldOfView({bare}) == nil)
        assert(Camera.GetClipPlanes({bare}) == nil)
    "#
    );
    run(&scene, &code).unwrap();
    let s = scene.borrow();
    let c = s.world.camera(cam).unwrap();
    assert_eq!((c.render_order, c.clear_flags), (1, ClearFlags::DepthOnly));
    assert_eq!((c.fov, c.far), (55.0, 100.0));
}

#[test]
fn lens_setters_clamp_like_unity() {
    let (scene, cam, _) = scene_with_camera();
    let code = format!(
        r#"
        Camera.SetFieldOfView({cam}, 400)
        assert(Camera.GetFieldOfView({cam}) == 179)
        Camera.SetClipPlanes({cam}, 0, 0)
        local near, far = Camera.GetClipPlanes({cam})
        assert(near > 0 and far > near, near .. " " .. far)
    "#
    );
    run(&scene, &code).unwrap();
}

#[test]
fn an_unknown_clear_flag_is_an_error_naming_the_choices() {
    let (scene, cam, _) = scene_with_camera();
    let err = run(
        &scene,
        &format!(r#"Camera.SetClearFlags({cam}, "Nothing")"#),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("Skybox, SolidColor, DepthOnly"), "{err}");
    let flags = scene.borrow().world.camera(cam).unwrap().clear_flags;
    assert_eq!(flags, ClearFlags::Skybox, "a rejected name changes nothing");
}
