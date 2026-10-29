//! The Screen Space Reflections section: the switch and, while on, the quality.

use super::{toggle, Section};
use crate::scene::authoring::rendering::post_processing::ssr;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "ssr",
    group: None,
    order: 40,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Screen Space Reflections (SSR)");
    let on = vc.ssr.active;
    toggle(
        ui,
        (&mut *world, id),
        "  SSR Active",
        on,
        ssr::set_active,
        is_dirty,
    );
    if !on {
        return;
    }
    let mut quality = vc.ssr.quality.clone();
    ui.horizontal(|ui| {
        ui.label("    Quality:");
        egui::ComboBox::from_label("")
            .selected_text(&quality)
            .show_ui(ui, |ui| {
                for q in ["Low", "Medium", "High", "Ultra"] {
                    if ui
                        .selectable_value(&mut quality, q.to_string(), q)
                        .changed()
                    {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            ssr::set_quality(&mut c, quality.clone());
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}
