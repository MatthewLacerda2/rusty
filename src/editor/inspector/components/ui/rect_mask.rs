//! The Rect Mask and Mask inspector cards (#418, #428): the rect clip's padding and
//! feather, and the graphic clip's show-graphic toggle. Writes route through
//! `scene::authoring::rect_mask`.

use egui_phosphor::regular as icon;

use super::vec4_row;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rect_mask as mask_ops;

/// Rect Mask card. A THIN client (#287): the widget reads a snapshot and routes the
/// write through the shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some((padding, mut feather)) = world.rect_mask(id).map(|m| (m.padding, m.feather)) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::CROP, "Rect Mask", Some(&mut remove), |ui| {
        if let Some(p) = vec4_row(ui, "Padding:", padding) {
            if let Some(mut m) = world.rect_mask_mut(id) {
                mask_ops::set_padding(&mut m, p);
            }
            changed = true;
        }
        let soft = ui
            .horizontal(|ui| {
                ui.label("Feather:");
                let drag = egui::DragValue::new(&mut feather).clamp_range(0.0..=f32::MAX);
                ui.add(drag).changed()
            })
            .inner;
        if let (true, Some(mut m)) = (soft, world.rect_mask_mut(id)) {
            mask_ops::set_feather(&mut m, feather);
            changed = true;
        }
    });
    if remove {
        world.set_rect_mask(id, None);
    }
    *is_dirty |= remove || changed;
}

/// Mask card: whether the mask graphic also draws. Same thin-client shape.
pub fn draw_mask(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut show) = world.mask(id).map(|m| m.show_mask_graphic) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::CIRCLE_DASHED, "Mask", Some(&mut remove), |ui| {
        let toggled = ui.checkbox(&mut show, "Show Mask Graphic").changed();
        if let (true, Some(mut m)) = (toggled, world.mask_mut(id)) {
            mask_ops::set_show_mask_graphic(&mut m, show);
            changed = true;
        }
    });
    if remove {
        world.set_mask(id, None);
    }
    *is_dirty |= remove || changed;
}
