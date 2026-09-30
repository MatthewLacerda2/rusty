//! Integration coverage for the in-game UI surface (#417): the `Canvas`,
//! `RectTransform` and `UI` namespaces drive the same components the layout reads,
//! `UI.GetRect` follows a script's change immediately, and both components survive
//! a scene save → load.

use std::cell::RefCell;

use glam::{Vec2, Vec3};
use mlua::{Lua, Table};
use rusty::components::{CanvasComponent, RectTransformComponent};
use rusty::core::video::VideoSettings;
use rusty::scene::{Camera, Scene};
use rusty::ui::ScreenSize;

/// A canvas with one centred 100×100 child. Returns `(scene, canvas, child)`.
fn ui_scene() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let canvas = scene.add_entity("Canvas".to_string());
    let c = Some(CanvasComponent::default());
    scene.world.set_canvas(canvas, c);
    let child = scene.add_entity("Panel".to_string());
    let r = Some(RectTransformComponent::default());
    scene.world.set_rect_transform(child, r);
    scene.set_parent(child, Some(canvas)).unwrap();
    (scene, canvas, child)
}

/// Run `f` with the three UI namespaces registered over `scene` on a 1920×1080
/// headless screen (the video resolution stands in for the game view).
fn with_ui_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    let screen = RefCell::new(ScreenSize::default());
    let video = RefCell::new(VideoSettings {
        width: 1920,
        height: 1080,
        ..VideoSettings::default()
    });
    let camera = RefCell::new(Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0));
    lua.scope(|scope| {
        rusty::api::canvas::register(&lua, scope, scene, &screen, &video).unwrap();
        rusty::api::rect_transform::register(&lua, scope, scene).unwrap();
        let (screen, video) = (&screen, &video);
        rusty::api::ui::register(&lua, scope, scene, (screen, video), &camera).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_scripted_rect_change_shows_in_get_rect_at_once() {
    let (scene, canvas, child) = ui_scene();
    let scene = RefCell::new(scene);
    with_ui_api(&scene, |lua| {
        // Stretch across the bottom, 80 tall, inset 10 from each side.
        lua.load(format!(
            "RectTransform.SetAnchorMin({child}, 0, 0)
             RectTransform.SetAnchorMax({child}, 1, 0)
             RectTransform.SetPivot({child}, 0.5, 0)
             RectTransform.SetAnchoredPosition({child}, 0, 10)
             RectTransform.SetSizeDelta({child}, -20, 80)"
        ))
        .exec()
        .unwrap();
        let r: Table = lua
            .load(format!("return UI.GetRect({child})"))
            .eval()
            .unwrap();
        assert_eq!(r.get::<_, f32>("x").unwrap(), 10.0);
        assert_eq!(r.get::<_, f32>("y").unwrap(), 10.0);
        assert_eq!(r.get::<_, f32>("width").unwrap(), 1900.0);
        assert_eq!(r.get::<_, f32>("height").unwrap(), 80.0);
        assert_eq!(r.get::<_, u32>("canvas").unwrap(), canvas);
        let screen: Table = r.get("screen").unwrap();
        assert_eq!(screen.get::<_, f32>("width").unwrap(), 1900.0);
        let corners: Table = r.get("corners").unwrap();
        assert_eq!(corners.raw_len(), 4);
        let (w, h): (f32, f32) = lua.load("return UI.GetScreenSize()").eval().unwrap();
        assert_eq!((w, h), (1920.0, 1080.0));
    });
}

#[test]
fn canvas_setters_validate_and_drive_the_scale_factor() {
    let (scene, canvas, child) = ui_scene();
    let scene = RefCell::new(scene);
    with_ui_api(&scene, |lua| {
        lua.load(format!(
            "Canvas.SetReferenceResolution({canvas}, 960, 540)
             Canvas.SetMatchWidthOrHeight({canvas}, 5)
             Canvas.SetSortOrder({canvas}, 3)
             Canvas.SetRenderMode({canvas}, 'Holographic')"
        ))
        .exec()
        .unwrap();
        let s: f32 = lua
            .load(format!("return Canvas.GetScaleFactor({canvas})"))
            .eval()
            .unwrap();
        assert!((s - 2.0).abs() < 1e-5);
        let m: f32 = lua
            .load(format!("return Canvas.GetMatchWidthOrHeight({canvas})"))
            .eval()
            .unwrap();
        assert_eq!(m, 1.0);
        let mode: String = lua
            .load(format!("return Canvas.GetRenderMode({canvas})"))
            .eval()
            .unwrap();
        assert_eq!(mode, "ScreenSpaceOverlay");
        // The centred 100×100 child now covers 200×200 screen pixels.
        let w: f32 = lua
            .load(format!("return UI.GetRect({child}).screen.width"))
            .eval()
            .unwrap();
        assert!((w - 200.0).abs() < 1e-3);
        let none: Option<Table> = lua.load("return UI.GetRect(999)").eval().unwrap();
        assert!(none.is_none());
    });
    assert_eq!(scene.borrow().world.canvas(canvas).unwrap().sort_order, 3);
}

#[test]
fn canvas_and_rect_transform_survive_save_and_load() {
    let (mut scene, canvas, child) = ui_scene();
    if let Some(mut r) = scene.world.rect_transform_mut(child) {
        r.anchored_position = Vec2::new(12.0, -4.0);
    }
    if let Some(mut c) = scene.world.canvas_mut(canvas) {
        c.sort_order = -2;
    }
    let path = std::env::temp_dir()
        .join(format!("rusty_ui_api_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    let rt = loaded.world.rect_transform(child).map(|r| r.clone());
    assert_eq!(rt.unwrap().anchored_position, Vec2::new(12.0, -4.0));
    assert_eq!(loaded.world.canvas(canvas).unwrap().sort_order, -2);
}
