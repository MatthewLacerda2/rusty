//! The UI's texture sizes follow the file (#1221): an Image laid out before its
//! PNG is baked measures the real size once it is, and a re-bake at a new
//! resolution moves the layout with it.

use glam::Vec2;

use super::refresh_texture_sizes;
use crate::components::{
    CanvasComponent, ImageComponent, LayoutAxisFit, LayoutElementComponent, RectTransformComponent,
};
use crate::core::fs::written::contract::{follows_rebakes, Bake};
use crate::procgen::{bake::bake_to_png, Image, Slot};
use crate::scene::Scene;
use crate::ui::{TextureSizes, UiLayout, UiView};

/// A canvas holding one Image of `texture`, fitted to its preferred size.
fn fitted_image(texture: &str) -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
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
    dir.join(format!("{name}.png"))
        .to_string_lossy()
        .into_owned()
}

/// The Image's laid-out size after the platform refreshed `sizes`.
fn laid_out(scene: &Scene, id: u32, sizes: &mut TextureSizes) -> Vec2 {
    refresh_texture_sizes(&scene.world, sizes);
    let view = UiView::screen(Vec2::new(1920.0, 1080.0)).with_textures(sizes.clone());
    let layout = UiLayout::compute_in(&scene.world, &view);
    layout.get(id).expect("laid out").rect.1
}

#[test]
fn an_image_follows_its_bakes() {
    let path = temp_png("icon");
    let (scene, id) = fitted_image(&path);
    let mut sizes = TextureSizes::new();
    let res = |which| if which == Bake::A { 8 } else { 16 };
    follows_rebakes(
        &path,
        |p, which| {
            bake_to_png(&Image::new(res(which)), Slot::BaseColor, p).expect("bake");
        },
        |_| laid_out(&scene, id, &mut sizes),
        |&size, which| size == Vec2::splat(res(which) as f32),
    );
}

#[test]
fn only_the_scenes_images_are_kept() {
    let path = temp_png("kept");
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    let (mut scene, id) = fitted_image(&path);
    let mut sizes = TextureSizes::new();
    sizes.set("gone.png", Vec2::ONE);
    assert_eq!(laid_out(&scene, id, &mut sizes), Vec2::splat(4.0));
    assert_eq!(sizes.get("gone.png"), None);
    scene.world.set_image(id, None);
    refresh_texture_sizes(&scene.world, &mut sizes);
    assert_eq!(sizes.get(&path), None);
}
