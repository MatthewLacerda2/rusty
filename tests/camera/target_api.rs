//! The per-entity `Camera` functions (#430): projection and render-texture target
//! through the Lua surface, onto the entity's `CameraComponent`.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, Projection};
use rusty::scene::Scene;

fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        let table = lua.create_table()?;
        rusty::api::camera::component::register(scope, &table, scene).unwrap();
        lua.globals().set("Camera", table)?;
        f(&lua);
        Ok(())
    })
    .unwrap();
}

fn scene_with_camera() -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Cam".to_string());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    let bare = scene.add_entity("Bare".to_string());
    (RefCell::new(scene), cam, bare)
}

#[test]
fn target_texture_round_trips_and_clears() {
    let (scene, cam, bare) = scene_with_camera();
    with_api(&scene, |lua| {
        let code = format!(
            r#"
            assert(Camera.GetTargetTexture({cam}) == nil)
            Camera.SetTargetTexture({cam}, "minimap", 128, 64)
            Camera.SetTargetPostFx({cam}, false)
            Camera.SetTargetUpdateEvery({cam}, 3)
            local n, w, h = Camera.GetTargetTexture({cam})
            assert(n == "minimap" and w == 128 and h == 64, n)
            assert(Camera.GetTargetPostFx({cam}) == false)
            assert(Camera.GetTargetUpdateEvery({cam}) == 3)
            -- Renaming keeps the size when omitted.
            Camera.SetTargetTexture({cam}, "scope")
            n, w, h = Camera.GetTargetTexture({cam})
            assert(n == "scope" and w == 128 and h == 64)
            -- No camera: nil getters, no-op setters.
            Camera.SetTargetTexture({bare}, "x", 8, 8)
            assert(Camera.GetTargetTexture({bare}) == nil)
            assert(Camera.GetProjection({bare}) == nil)
        "#
        );
        lua.load(&code).exec().unwrap();
    });
    let s = scene.borrow();
    let t = s.world.camera(cam).unwrap().target_texture.clone().unwrap();
    assert_eq!(
        (t.path().as_str(), t.update_every, t.post_fx),
        ("rt:scope", 3, false)
    );
    drop(s);
    with_api(&scene, |lua| {
        lua.load(format!("Camera.SetTargetTexture({cam}, nil)"))
            .exec()
            .unwrap();
    });
    assert!(scene
        .borrow()
        .world
        .camera(cam)
        .unwrap()
        .target_texture
        .is_none());
}

#[test]
fn projection_switches_to_orthographic_and_back() {
    let (scene, cam, _) = scene_with_camera();
    with_api(&scene, |lua| {
        let code = format!(
            r#"
            local name, size = Camera.GetProjection({cam})
            assert(name == "Perspective" and size == nil)
            Camera.SetProjection({cam}, "orthographic", 40)
            name, size = Camera.GetProjection({cam})
            assert(name == "Orthographic" and size == 40)
            Camera.SetProjection({cam}, "Fisheye") -- unknown: ignored
            assert(Camera.GetProjection({cam}) == "Orthographic")
        "#
        );
        lua.load(&code).exec().unwrap();
    });
    let p = scene.borrow().world.camera(cam).unwrap().projection;
    assert_eq!(p, Projection::Orthographic { size: 40.0 });
}
