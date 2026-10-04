//! src/editor/inspector/components/add/ui.rs — the in-game UI half of the Add
//! Component menu: Canvas, RectTransform, the graphics, masks and auto-layout.

use egui_phosphor::regular as icon;

use crate::components::{CanvasComponent, CanvasGroupComponent, RectTransformComponent};
use crate::scene::authoring::{self, ComponentKind};

/// Add-menu entries for the in-game UI (#417, #418, #419, #420): a Canvas makes the entity
/// a UI root; a RectTransform makes it a UI element laid out inside its parent's
/// rect; an Image draws it; a Text labels it; a Canvas Group fades its subtree; a
/// Rect Mask clips it; a Shape draws a texture-free SDF primitive (#425); a
/// Selectable makes it interactive (#420). Each is offered only
/// when absent. Image, Text, Shape, Rect Mask and Selectable declare `requires(RectTransform)`, so they go through the shared dependency
/// verb, matching `Scene.AddComponent`.
pub(super) fn add_ui_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_canvas(id) && ui.button(format!("{}  Canvas", icon::MONITOR)).clicked() {
        world.set_canvas(id, Some(CanvasComponent::default()));
        ui.close();
    }
    if !world.has_rect_transform(id)
        && ui
            .button(format!("{}  Rect Transform", icon::FRAME_CORNERS))
            .clicked()
    {
        world.set_rect_transform(id, Some(RectTransformComponent::default()));
        ui.close();
    }
    if !world.has_image(id) && ui.button(format!("{}  Image", icon::IMAGE)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::Image);
        ui.close();
    }
    if !world.has_canvas_group(id)
        && ui
            .button(format!("{}  Canvas Group", icon::STACK))
            .clicked()
    {
        world.set_canvas_group(id, Some(CanvasGroupComponent::default()));
        ui.close();
    }
    if !world.has_text(id) && ui.button(format!("{}  Text", icon::TEXT_T)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::Text);
        ui.close();
    }
    if !world.has_shape(id) && ui.button(format!("{}  Shape", icon::SHAPES)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::Shape);
        ui.close();
    }
    if !world.has_rect_mask(id) && ui.button(format!("{}  Rect Mask", icon::CROP)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::RectMask);
        ui.close();
    }
    add_mask_components(ui, world, id);
    if !world.has_selectable(id)
        && ui
            .button(format!("{}  Selectable", icon::CURSOR_CLICK))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::Selectable);
        ui.close();
    }
}

/// Add-menu entries for the graphic-shaped Mask (#428) and the Backdrop Filter
/// (#426). Both require a RectTransform, via the shared dependency verb.
fn add_mask_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    type Has = fn(&crate::ecs::World, u32) -> bool;
    let entries: [(ComponentKind, Has, &str, &str); 2] = [
        (
            ComponentKind::Mask,
            |w, id| w.has_mask(id),
            icon::CIRCLE_DASHED,
            "Mask",
        ),
        (
            ComponentKind::BackdropFilter,
            |w, id| w.has_backdrop_filter(id),
            icon::DROP_HALF,
            "Backdrop Filter",
        ),
    ];
    for (kind, has, glyph, name) in entries {
        if !has(world, id) && ui.button(format!("{glyph}  {name}")).clicked() {
            authoring::add_with_requirements(world, id, kind);
            ui.close();
        }
    }
}

/// Add-menu entries for UI auto-layout (#421): a Layout Group arranges the children
/// in a row, column or grid; a Layout Element overrides an element's layout sizes
/// and fits it to its content. Both require a RectTransform, via the shared
/// dependency verb.
pub(super) fn add_layout_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_layout_group(id)
        && ui
            .button(format!("{}  Layout Group", icon::LAYOUT))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::LayoutGroup);
        ui.close();
    }
    if !world.has_layout_element(id)
        && ui
            .button(format!("{}  Layout Element", icon::ARROWS_OUT))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::LayoutElement);
        ui.close();
    }
}
