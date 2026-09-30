//! src/scene/authoring/ui_widgets/scroll.rs — `Scroll View`.
//!
//! `Viewport` (a `RectMask`) over `Content` — pinned to the viewport's top-left,
//! a vertical `LayoutGroup` whose `LayoutElement` fits its height to its items —
//! plus a horizontal and a vertical `Scrollbar`. `scroll_view.lua` moves
//! `Content`'s anchored position and keeps the bars in step.

use glam::{Vec2, Vec4};

use super::parts::{image, node, script, Place};
use super::range::scrollbar_sized;
use crate::components::{
    LayoutAxisFit, LayoutElementComponent, LayoutGroupComponent, LayoutKind, RectMaskComponent,
};
use crate::scene::Scene;

/// The bar thickness, reference units.
pub const BAR: f32 = 20.0;

/// `Scroll View` — 400×400 by default.
pub fn scroll_view(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Scroll View",
        parent,
        Place::centred(Vec2::splat(400.0)),
    );
    image(scene, id, Vec4::new(1.0, 1.0, 1.0, 0.392));
    build_into(scene, id, true);
    script(scene, id, "scroll_view");
    id
}

/// Add the viewport, content, the vertical bar and (with `horizontal`) the
/// horizontal one under `id` — the dropdown's list reuses this without it.
/// Returns the `Content` id.
pub fn build_into(scene: &mut Scene, id: u32, horizontal: bool) -> u32 {
    let bottom_bar = if horizontal { BAR } else { 0.0 };
    let top_left = Vec2::new(0.0, 1.0);
    let viewport = node(
        scene,
        "Viewport",
        Some(id),
        Place(
            Vec2::ZERO,
            Vec2::ONE,
            top_left,
            Vec2::ZERO,
            Vec2::new(-BAR, -bottom_bar),
        ),
    );
    image(scene, viewport, Vec4::new(1.0, 1.0, 1.0, 0.0));
    scene
        .world
        .set_rect_mask(viewport, Some(RectMaskComponent::default()));
    let content = node(
        scene,
        "Content",
        Some(viewport),
        Place(
            top_left,
            Vec2::ONE,
            top_left,
            Vec2::ZERO,
            Vec2::new(0.0, 300.0),
        ),
    );
    fit_content(scene, content);
    bars(scene, id, horizontal);
    content
}

/// `Content` lists its children top to bottom and fits its height to them.
fn fit_content(scene: &mut Scene, content: u32) {
    let group = LayoutGroupComponent {
        kind: LayoutKind::Vertical,
        control_child_width: true,
        control_child_height: true,
        child_force_expand_width: true,
        child_force_expand_height: false,
        ..Default::default()
    };
    scene.world.set_layout_group(content, Some(group));
    let fit = LayoutElementComponent {
        vertical_fit: LayoutAxisFit::PreferredSize,
        ..Default::default()
    };
    scene.world.set_layout_element(content, Some(fit));
}

/// The vertical bar down the right and (with `horizontal`) one along the bottom.
fn bars(scene: &mut Scene, id: u32, horizontal: bool) {
    let bottom_bar = if horizontal { BAR } else { 0.0 };
    if horizontal {
        let bottom = Place(
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(-BAR, BAR),
        );
        let h = scrollbar_sized(scene, Some(id), bottom, None);
        scene.world.set_name(h, "Scrollbar Horizontal".to_string());
    }
    let right = Place(
        Vec2::new(1.0, 0.0),
        Vec2::ONE,
        Vec2::ONE,
        Vec2::ZERO,
        Vec2::new(BAR, -bottom_bar),
    );
    let v = scrollbar_sized(scene, Some(id), right, Some("BottomToTop"));
    scene.world.set_name(v, "Scrollbar Vertical".to_string());
}
