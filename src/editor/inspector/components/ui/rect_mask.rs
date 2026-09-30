//! The Rect Mask inspector card (#418): the clip's padding. The write routes
//! through `scene::authoring::rect_mask`.

use egui_phosphor::regular as icon;

use super::vec4_row;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rect_mask as mask_ops;

/// Rect Mask card. A THIN client (#287): the widget reads a snapshot and routes the
/// write through the shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(padding) = world.rect_mask(id).map(|m| m.padding) else {
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
    });
    if remove {
        world.set_rect_mask(id, None);
    }
    *is_dirty |= remove || changed;
}
