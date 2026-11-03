//! Integration coverage for the `Text` namespace (#419): setters write through the
//! shared ops (validation included), the layout reads answer from the CPU layout
//! in the element's rect, and the component survives a scene save → load.

mod layout;
mod setters;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec2;
use mlua::Lua;
use rusty::api::bind;
use rusty::components::{CanvasComponent, RectTransformComponent, TextComponent};
use rusty::core::settings::video::VideoSettings;
use rusty::scene::Scene;
use rusty::ui::ScreenSize;

/// A canvas with one 400×100 Text child; returns the scene and the label's id.
pub(super) fn label() -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    let id = scene.add_entity("Label".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(400.0, 100.0),
        ..Default::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.world.set_text(id, Some(TextComponent::default()));
    scene.set_parent(id, Some(root)).unwrap();
    (scene, id)
}

/// Run `f` with `Text` registered over `scene` on a 1920×1080 headless screen.
pub(super) fn with_api(scene: &Rc<RefCell<Scene>>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    let screen = Rc::new(RefCell::new(ScreenSize::default()));
    let video = Rc::new(RefCell::new(VideoSettings {
        width: 1920,
        height: 1080,
        ..VideoSettings::default()
    }));
    rusty::api::text::register(
        &lua,
        bind(&lua, scene),
        bind(&lua, &screen),
        bind(&lua, &video),
    )
    .unwrap();
    f(&lua);
}

#[test]
fn text_survives_save_and_load() {
    let (mut scene, id) = label();
    let text = TextComponent {
        text: "<b>Objective</b>: reach the <color=#ffcc00>roof</color>".into(),
        font: Some("fonts/hud.ttf".into()),
        glow_size: 0.1,
        shadow_offset: Vec2::new(1.0, -1.0),
        ..Default::default()
    };
    scene.world.set_text(id, Some(text.clone()));
    let path = crate::temp::dir()
        .join(format!("rusty_ui_text_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(loaded.world.text(id).map(|t| t.clone()), Some(text));
}
