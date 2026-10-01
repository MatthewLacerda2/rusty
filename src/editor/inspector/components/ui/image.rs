//! The Image inspector card (#418): tint, texture, image type and the per-type
//! fields (9-slice border, fill, preserve aspect), the raycast target, and the
//! look (#425): a gradient tint and the blend mode. Widgets
//! edit a snapshot; every write routes through `scene::authoring::image`.

use egui_phosphor::regular as icon;

use super::look::{blend_row, gradient_editor};
use super::{combo, vec4_row};
use crate::components::{FillMethod, FillOrigin, ImageComponent, ImageType};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::image as image_ops;

/// Image card. A THIN client (#287): widgets edit a snapshot and the changed
/// snapshot is written back field by field through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.image(id).map(|i| i.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::IMAGE, "Image", Some(&mut remove), |ui| {
        changed |= draw_common(ui, &mut edit);
        changed |= match edit.image_type {
            ImageType::Simple => ui
                .checkbox(&mut edit.preserve_aspect, "Preserve Aspect")
                .changed(),
            ImageType::Sliced => vec4_row(ui, "Border:", edit.border)
                .map(|b| edit.border = b)
                .is_some(),
            ImageType::Tiled => false,
            ImageType::Filled => draw_fill(ui, &mut edit),
        };
        changed |= ui
            .checkbox(&mut edit.raycast_target, "Raycast Target")
            .changed();
        changed |= gradient_editor(ui, &mut edit.gradient);
        changed |= blend_row(ui, &mut edit.blend);
        changed |= super::shader::row(ui, &mut edit.shader);
    });
    if changed {
        if let Some(mut i) = world.image_mut(id) {
            write_back(&mut i, edit);
        }
    }
    if remove {
        world.set_image(id, None);
    }
    *is_dirty |= remove || changed;
}

/// Colour, texture path and image type.
fn draw_common(ui: &mut egui::Ui, edit: &mut ImageComponent) -> bool {
    let mut color = edit.color.to_array();
    let mut changed = ui
        .horizontal(|ui| {
            ui.label("Color:");
            ui.color_edit_button_rgba_unmultiplied(&mut color).changed()
        })
        .inner;
    edit.color = glam::Vec4::from_array(color);
    let mut path = edit.texture.clone().unwrap_or_default();
    changed |= ui
        .horizontal(|ui| {
            ui.label("Texture:");
            ui.text_edit_singleline(&mut path).changed()
        })
        .inner;
    edit.texture = Some(path);
    let types = [
        ImageType::Simple,
        ImageType::Sliced,
        ImageType::Tiled,
        ImageType::Filled,
    ];
    changed |= combo(ui, "Image Type", &mut edit.image_type, &types, |t| {
        image_ops::image_type_name(t)
    });
    changed
}

/// Fill method, origin, amount and direction.
fn draw_fill(ui: &mut egui::Ui, edit: &mut ImageComponent) -> bool {
    let methods = [
        FillMethod::Horizontal,
        FillMethod::Vertical,
        FillMethod::Radial360,
    ];
    let origins = [
        FillOrigin::Left,
        FillOrigin::Right,
        FillOrigin::Bottom,
        FillOrigin::Top,
    ];
    let mut changed = combo(ui, "Fill Method", &mut edit.fill_method, &methods, |m| {
        image_ops::fill_method_name(m)
    });
    changed |= combo(ui, "Fill Origin", &mut edit.fill_origin, &origins, |o| {
        image_ops::fill_origin_name(o)
    });
    changed |= ui
        .horizontal(|ui| {
            ui.label("Fill Amount:");
            ui.add(egui::Slider::new(&mut edit.fill_amount, 0.0..=1.0))
                .changed()
        })
        .inner;
    if edit.fill_method == FillMethod::Radial360 {
        changed |= ui.checkbox(&mut edit.fill_clockwise, "Clockwise").changed();
    }
    changed
}

/// Write the edited snapshot back through the shared ops (method before origin, so
/// the origin is fitted to the new method).
fn write_back(i: &mut ImageComponent, edit: ImageComponent) {
    image_ops::set_color(i, edit.color);
    image_ops::set_texture(i, edit.texture);
    image_ops::set_image_type(i, edit.image_type);
    image_ops::set_border(i, edit.border);
    image_ops::set_fill_method(i, edit.fill_method);
    image_ops::set_fill_origin(i, edit.fill_origin);
    image_ops::set_fill_amount(i, edit.fill_amount);
    image_ops::set_fill_clockwise(i, edit.fill_clockwise);
    image_ops::set_preserve_aspect(i, edit.preserve_aspect);
    image_ops::set_raycast_target(i, edit.raycast_target);
    image_ops::set_gradient(i, edit.gradient);
    image_ops::set_blend(i, edit.blend);
    super::shader::write_back(&mut i.shader, edit.shader);
}
