//! src/scene/authoring/ui_widgets/input.rs — `Input Field`.
//!
//! A white box whose masked `Text Area` holds, back to front: `Selection` (the
//! highlight rects the script adds, one per selected line), `Placeholder`, `Text`
//! (plain, unwrapped — the script scrolls it) and the `Caret`.

use glam::{Vec2, Vec4};

use super::parts::{image, node, script, selectable, text, Place, DARK, WHITE};
use crate::components::{RectMaskComponent, TextAlignment};
use crate::scene::Scene;

/// `Input Field` — 320×60.
pub fn input_field(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Input Field",
        parent,
        Place::centred(Vec2::new(320.0, 60.0)),
    );
    image(scene, id, WHITE);
    text_area(scene, id);
    selectable(scene, id, None);
    script(scene, id, "input_field");
    id
}

/// The masked `Text Area` and its four layers, under `id`.
fn text_area(scene: &mut Scene, id: u32) {
    let area = node(
        scene,
        "Text Area",
        Some(id),
        Place::fill(Vec2::new(10.0, 6.0)),
    );
    scene
        .world
        .set_rect_mask(area, Some(RectMaskComponent::default()));
    node(scene, "Selection", Some(area), Place::fill(Vec2::ZERO));
    let hint = node(scene, "Placeholder", Some(area), Place::fill(Vec2::ZERO));
    text(
        scene,
        hint,
        "Enter text...",
        28.0,
        TextAlignment::MiddleLeft,
    );
    if let Some(mut t) = scene.world.text_mut(hint) {
        t.color = Vec4::new(0.196, 0.196, 0.196, 0.5);
    }
    let body = node(
        scene,
        "Text",
        Some(area),
        Place(
            Vec2::ZERO,
            Vec2::ONE,
            Vec2::new(0.0, 0.5),
            Vec2::ZERO,
            Vec2::ZERO,
        ),
    );
    text(scene, body, "", 28.0, TextAlignment::MiddleLeft);
    let left = Vec2::new(0.0, 0.5);
    let caret = node(
        scene,
        "Caret",
        Some(area),
        Place(left, left, left, Vec2::ZERO, Vec2::new(3.0, 34.0)),
    );
    image(scene, caret, DARK);
    if let Some(mut img) = scene.world.image_mut(caret) {
        img.color.w = 0.0;
        img.raycast_target = false;
    }
}
