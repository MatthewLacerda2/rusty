//! src/scene/authoring/ui_widgets/dropdown.rs — `Dropdown`.
//!
//! A button showing the chosen option's `Label`, an `Arrow`, and an inactive
//! `Template`: a scroll view (vertical bar only) whose `Content` the script fills
//! with one `Item` per option when it opens. Open, the template is lifted onto a
//! popup canvas above everything (see `dropdown.lua`).

use glam::{Vec2, Vec4};

use super::parts::{image, label, node, script, selectable, Place, DARK, WHITE};
use super::scroll::build_into;
use crate::components::TextAlignment;
use crate::scene::Scene;

/// `Dropdown` — 320×60, with a 300-tall list template below it.
pub fn dropdown(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Dropdown",
        parent,
        Place::centred(Vec2::new(320.0, 60.0)),
    );
    image(scene, id, WHITE);
    let l = label(
        scene,
        id,
        "Option A",
        28.0,
        TextAlignment::MiddleLeft,
        Vec2::ZERO,
    );
    if let Some(mut rt) = scene.world.rect_transform_mut(l) {
        rt.anchored_position.x = -10.0;
        rt.size_delta.x = -60.0;
    }
    let right = Vec2::new(1.0, 0.5);
    let arrow = node(
        scene,
        "Arrow",
        Some(id),
        Place(
            right,
            right,
            right,
            Vec2::new(-15.0, 0.0),
            Vec2::splat(20.0),
        ),
    );
    image(scene, arrow, DARK);
    template(scene, id);
    selectable(scene, id, None);
    script(scene, id, "dropdown");
    id
}

/// The inactive list template: a vertical-only scroll view hung below `id`.
fn template(scene: &mut Scene, id: u32) {
    let template = node(
        scene,
        "Template",
        Some(id),
        Place(
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 1.0),
            Vec2::new(0.0, -4.0),
            Vec2::new(0.0, 300.0),
        ),
    );
    image(scene, template, Vec4::new(0.95, 0.95, 0.95, 1.0));
    build_into(scene, template, false);
    script(scene, template, "scroll_view");
    scene.world.set_active(template, false);
}
