//! The UI's texture sizes follow the file (#1221): an Image laid out before its
//! PNG is baked measures the real size once it is, a re-bake at a new resolution
//! moves the layout with it, and a file that appears outside the engine is found
//! within the miss window.

use std::rc::Rc;

use glam::Vec2;

use super::TextureSizeReader;
use crate::components::{
    CanvasComponent, ImageComponent, ImageType, LayoutAxisFit, LayoutElementComponent,
    RectTransformComponent,
};
use crate::core::fs::freshness::MISS_RETRY_FRAMES;
use crate::core::fs::written::contract::{follows_rebakes, Bake};
use crate::procgen::{bake::bake_to_png, Image, Slot};
use crate::scene::Scene;
use crate::ui::{TextureSizes, UiLayout, UiView};

/// A canvas holding one Image of `texture`, fitted to its preferred size.
fn fitted_image(texture: &str) -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent::default();
    scene.world.set_canvas(root, Some(canvas));
    let id = scene.add_entity("Icon".to_string());
    let rt = RectTransformComponent::default();
    scene.world.set_rect_transform(id, Some(rt));
    let fit = LayoutElementComponent {
        horizontal_fit: LayoutAxisFit::PreferredSize,
        vertical_fit: LayoutAxisFit::PreferredSize,
        ..LayoutElementComponent::default()
    };
    scene.world.set_layout_element(id, Some(fit));
    let img = ImageComponent {
        texture: Some(texture.to_string()),
        ..ImageComponent::default()
    };
    scene.world.set_image(id, Some(img));
    scene.set_parent(id, Some(root)).expect("parent exists");
    (scene, id)
}

fn temp_png(name: &str) -> String {
    let dir = crate::test_temp::dir().join("rusty_1221");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("{name}.png"));
    std::fs::remove_file(&path).ok();
    path.to_string_lossy().into_owned()
}

/// One tick's worth: refresh the table, then lay the Image out with it.
fn tick(
    scene: &Scene,
    id: u32,
    (reader, sizes): &mut (TextureSizeReader, Rc<TextureSizes>),
) -> Vec2 {
    reader.refresh(&scene.world, sizes);
    let view = UiView::screen(Vec2::new(1920.0, 1080.0), Rc::clone(sizes));
    let layout = UiLayout::compute_in(&scene.world, &view);
    layout.get(id).expect("laid out").rect.1
}

#[test]
fn an_image_follows_its_bakes() {
    let path = temp_png("icon");
    let (scene, id) = fitted_image(&path);
    let mut state = (TextureSizeReader::new(), Rc::default());
    let res = |which| if which == Bake::A { 8 } else { 16 };
    follows_rebakes(
        &path,
        |p, which| {
            bake_to_png(&Image::new(res(which)), Slot::BaseColor, p).expect("bake");
        },
        |_| tick(&scene, id, &mut state),
        |&size, which| size == Vec2::splat(res(which) as f32),
    );
}

#[test]
fn a_file_written_outside_the_engine_appears_within_the_miss_window() {
    let path = temp_png("external");
    let (scene, id) = fitted_image(&path);
    let mut state = (TextureSizeReader::new(), Rc::default());
    assert_eq!(tick(&scene, id, &mut state), Vec2::ZERO);
    image::RgbaImage::new(6, 3).save(&path).expect("write png");
    let sizes = (0..MISS_RETRY_FRAMES).map(|_| tick(&scene, id, &mut state));
    assert_eq!(sizes.last(), Some(Vec2::new(6.0, 3.0)));
}

#[test]
fn only_the_scenes_measured_images_are_kept() {
    let path = temp_png("kept");
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    let (mut scene, id) = fitted_image(&path);
    let mut state = (TextureSizeReader::new(), Rc::<TextureSizes>::default());
    Rc::make_mut(&mut state.1).set("gone.png", Vec2::ONE);
    assert_eq!(tick(&scene, id, &mut state), Vec2::splat(4.0));
    assert_eq!(state.1.get("gone.png"), None);
    // A Sliced Image measures by its borders: its header is not read.
    let mut img = scene.world.image(id).map(|i| i.clone()).expect("image");
    img.image_type = ImageType::Sliced;
    scene.world.set_image(id, Some(img));
    tick(&scene, id, &mut state);
    assert_eq!(state.1.get(&path), None);
}

#[test]
fn a_bare_tick_reads_the_sizes_and_an_idle_one_copies_nothing() {
    use crate::{app::GameWorld, core::input::InputState, navigation::NavigationGraph};
    use std::cell::RefCell;
    let path = temp_png("ticked");
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    let (scene, _) = fitted_image(&path);
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(crate::scripting::ConsoleLogs::new())),
    );
    // No refresh call beside the tick: the tick does it (#1241).
    game.tick(crate::time::FIXED_DELTA_TIME);
    // Held as the renderer and the editor hold it, so a write would copy it.
    let held = Rc::clone(game.resources.screen.borrow().textures());
    assert_eq!(held.get(&path), Some(Vec2::splat(4.0)));
    game.tick(crate::time::FIXED_DELTA_TIME);
    let now = Rc::clone(game.resources.screen.borrow().textures());
    assert!(
        Rc::ptr_eq(&held, &now),
        "nothing changed: the table is not copied"
    );
}
