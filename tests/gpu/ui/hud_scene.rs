//! The HUD scene the UI screenshot test renders (#418): a canvas whose reference
//! resolution equals the shot (one reference unit = one pixel), over an opaque
//! black backdrop so every blend is against a known colour.

use glam::{Vec2, Vec4};
use rusty::components::{
    CanvasComponent, CanvasGroupComponent, FillMethod, ImageComponent, ImageType,
    RectMaskComponent, RectTransformComponent,
};
use rusty::scene::Scene;

pub const SIZE: u32 = 256;

/// A bottom-left-anchored element at `pos`, `size` reference units, under `parent`.
fn element(
    scene: &mut Scene,
    parent: u32,
    pos: Vec2,
    size: Vec2,
    image: Option<ImageComponent>,
) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: pos,
        size_delta: size,
        world_anchor: None,
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.world.set_image(id, image);
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

fn solid(r: f32, g: f32, b: f32) -> Option<ImageComponent> {
    Some(ImageComponent {
        color: Vec4::new(r, g, b, 1.0),
        ..Default::default()
    })
}

/// A 32×32 texture: an 8-texel green frame around a blue centre.
fn frame_texture() -> String {
    let path = crate::temp::dir().join("rusty_ui_frame.png");
    let img = image::RgbaImage::from_fn(32, 32, |x, y| {
        let edge = x < 8 || y < 8 || x >= 24 || y >= 24;
        image::Rgba(if edge {
            [0, 255, 0, 255]
        } else {
            [0, 0, 255, 255]
        })
    });
    img.save(&path).expect("write texture");
    path.to_string_lossy().into_owned()
}

/// The HUD: a 50% bar, a 9-sliced panel, a faded group and a clipped child.
pub fn hud() -> Scene {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    // An opaque black backdrop, so every blend below is against a known colour.
    let full = SIZE as f32;
    at(
        &mut scene,
        root,
        [0.0, 0.0, full, full],
        solid(0.0, 0.0, 0.0),
    );
    add_bar_and_panel(&mut scene, root);
    add_group_and_mask(&mut scene, root);
    scene
}

/// A red bar filled to 50% and a 9-sliced green-framed blue panel.
fn add_bar_and_panel(scene: &mut Scene, root: u32) {
    let bar = ImageComponent {
        color: Vec4::new(1.0, 0.0, 0.0, 1.0),
        image_type: ImageType::Filled,
        fill_method: FillMethod::Horizontal,
        fill_amount: 0.5,
        ..Default::default()
    };
    at(scene, root, [16.0, 16.0, 96.0, 16.0], Some(bar));
    let panel = ImageComponent {
        texture: Some(frame_texture()),
        image_type: ImageType::Sliced,
        border: Vec4::splat(8.0),
        ..Default::default()
    };
    at(scene, root, [128.0, 16.0, 112.0, 96.0], Some(panel));
}

/// A white square under a 50% CanvasGroup, and a yellow child overflowing a mask.
fn add_group_and_mask(scene: &mut Scene, root: u32) {
    let group = at(scene, root, [16.0, 128.0, 96.0, 96.0], None);
    let faded = CanvasGroupComponent {
        alpha: 0.5,
        ..Default::default()
    };
    scene.world.set_canvas_group(group, Some(faded));
    at(scene, group, [0.0, 0.0, 96.0, 96.0], solid(1.0, 1.0, 1.0));
    let mask = at(scene, root, [128.0, 128.0, 64.0, 64.0], None);
    let clip = Some(RectMaskComponent::default());
    scene.world.set_rect_mask(mask, clip);
    at(scene, mask, [32.0, 32.0, 96.0, 96.0], solid(1.0, 1.0, 0.0));
}

/// [`element`] with its rect as `[x, y, width, height]`.
fn at(scene: &mut Scene, parent: u32, r: [f32; 4], image: Option<ImageComponent>) -> u32 {
    element(
        scene,
        parent,
        Vec2::new(r[0], r[1]),
        Vec2::new(r[2], r[3]),
        image,
    )
}
