//! src/scene/authoring/ui_widgets/basic.rs — Canvas, Panel, Image, Text, Button,
//! Toggle and Toggle Group.

use glam::{Vec2, Vec4};

use super::parts::{image, label, node, script, selectable, text, Place, DARK, WHITE};
use crate::components::{CanvasComponent, LayoutGroupComponent, LayoutKind, TextAlignment};
use crate::scene::Scene;

/// A root `Canvas` with the default scaler (1920×1080 reference).
pub fn canvas(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = scene.add_entity("Canvas".to_string());
    scene.world.set_canvas(id, Some(CanvasComponent::default()));
    if let Some(p) = parent {
        let _ = scene.set_parent(id, Some(p));
    }
    id
}

/// A translucent white panel filling its parent (Unity's `Panel`).
pub fn panel(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(scene, "Panel", parent, Place::fill(Vec2::ZERO));
    image(scene, id, Vec4::new(1.0, 1.0, 1.0, 0.392));
    id
}

/// A 200×200 white `Image`.
pub fn image_node(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(scene, "Image", parent, Place::centred(Vec2::splat(200.0)));
    image(scene, id, WHITE);
    id
}

/// A 320×100 `Text`.
pub fn text_node(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Text",
        parent,
        Place::centred(Vec2::new(320.0, 100.0)),
    );
    text(scene, id, "New Text", 36.0, TextAlignment::TopLeft);
    if let Some(mut t) = scene.world.text_mut(id) {
        t.wrap = true;
        t.rich_text = true;
    }
    id
}

/// `Button` — an Image + Selectable + `button.lua`, with a centred label.
pub fn button(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Button",
        parent,
        Place::centred(Vec2::new(320.0, 60.0)),
    );
    image(scene, id, WHITE);
    selectable(scene, id, None);
    script(scene, id, "button");
    label(
        scene,
        id,
        "Button",
        28.0,
        TextAlignment::MiddleCenter,
        Vec2::ZERO,
    );
    id
}

/// `Toggle` — a box with a checkmark, a label, `toggle.lua`.
pub fn toggle(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Toggle",
        parent,
        Place::centred(Vec2::new(320.0, 40.0)),
    );
    let left = Vec2::new(0.0, 0.5);
    let bg = node(
        scene,
        "Background",
        Some(id),
        Place(left, left, left, Vec2::ZERO, Vec2::splat(40.0)),
    );
    image(scene, bg, WHITE);
    let check = node(scene, "Checkmark", Some(bg), Place::fill(Vec2::splat(8.0)));
    image(scene, check, DARK);
    let l = label(
        scene,
        id,
        "Toggle",
        28.0,
        TextAlignment::MiddleLeft,
        Vec2::ZERO,
    );
    if let Some(mut rt) = scene.world.rect_transform_mut(l) {
        rt.anchored_position.x = 26.0;
        rt.size_delta.x = -52.0;
    }
    // Clicking the label toggles too, as in Unity.
    if let Some(mut t) = scene.world.text_mut(l) {
        t.raycast_target = true;
    }
    selectable(scene, id, Some(bg));
    script(scene, id, "toggle");
    id
}

/// `Toggle Group` — a column of two toggles under `toggle_group.lua`: radio buttons.
pub fn toggle_group(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Toggle Group",
        parent,
        Place::centred(Vec2::new(320.0, 100.0)),
    );
    let group = LayoutGroupComponent {
        kind: LayoutKind::Vertical,
        spacing: Vec2::splat(20.0),
        control_child_width: true,
        control_child_height: false,
        child_force_expand_width: true,
        child_force_expand_height: false,
        ..Default::default()
    };
    scene.world.set_layout_group(id, Some(group));
    script(scene, id, "toggle_group");
    for n in 1..=2 {
        let t = toggle(scene, Some(id));
        scene.world.set_name(t, format!("Option {n}"));
        if let Some(l) = scene.world.children(t).last().copied() {
            if let Some(mut txt) = scene.world.text_mut(l) {
                txt.text = format!("Option {n}");
            }
        }
    }
    id
}
