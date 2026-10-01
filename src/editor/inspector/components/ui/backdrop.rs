//! The Backdrop Filter inspector card (#426): blur radius, tint, saturation and
//! brightness. Every write routes through `scene::authoring::backdrop`.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::backdrop as ops;

/// Backdrop Filter card. A THIN client (#287): widgets read a snapshot and route
/// each write through a shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut b) = world.backdrop_filter(id).map(|b| b.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::DROP_HALF,
        "Backdrop Filter",
        Some(&mut remove),
        |ui| {
            let mut tint = b.tint.to_array();
            let edited = ui
                .vertical(|ui| {
                    let r = drag(ui, "Blur Radius:", &mut b.blur_radius, 0.0..=256.0);
                    let t = ui
                        .horizontal(|ui| {
                            ui.label("Tint:");
                            ui.color_edit_button_rgba_unmultiplied(&mut tint).changed()
                        })
                        .inner;
                    let s = drag(ui, "Saturation:", &mut b.saturation, 0.0..=2.0);
                    let l = drag(ui, "Brightness:", &mut b.brightness, 0.0..=2.0);
                    r || t || s || l
                })
                .inner;
            if let (true, Some(mut f)) = (edited, world.backdrop_filter_mut(id)) {
                ops::set_blur_radius(&mut f, b.blur_radius);
                ops::set_tint(&mut f, glam::Vec4::from_array(tint));
                ops::set_saturation(&mut f, b.saturation);
                ops::set_brightness(&mut f, b.brightness);
                changed = true;
            }
        },
    );
    if remove {
        world.set_backdrop_filter(id, None);
    }
    *is_dirty |= remove || changed;
}

/// A labelled slider over `range`; true when it changed.
fn drag(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(v, range)).changed()
    })
    .inner
}
