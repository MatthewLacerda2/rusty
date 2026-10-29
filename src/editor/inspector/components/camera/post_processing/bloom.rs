//! The Bloom section: the switch and, while on, its knobs, the lens dirt texture
//! (#1442) and its quality level, with the `Custom` divisor (#1522).

use super::path_field::{self, DIRT};
use super::{count_row, knob, knob_row, quality_row, toggle, Section};
use crate::core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use crate::scene::authoring::rendering::post_processing::bloom;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "bloom",
    group: None,
    order: 10,
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
    ui.label("Bloom");
    let on = vc.bloom.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Bloom Active",
        on,
        bloom::set_active,
        is_dirty,
    );
    if on {
        let knobs = bloom::EFFECT.knobs;
        let rows = [
            ("    Intensity:", "intensity", 5.0),
            ("    Threshold:", "threshold", 1.0),
            ("    Dirt Intensity:", "dirt_intensity", 10.0),
        ];
        for (label, field, max) in rows {
            let row = (label, knob(knobs, field));
            *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, 0.0..=max);
        }
        path_field::draw(ui, (&mut *world, id), &DIRT, vc, is_dirty);
        if let Some(quality) = quality_row(ui, "Bloom Quality", vc.bloom.quality) {
            if let Some(mut c) = world.post_processing_mut(id) {
                bloom::set_quality(&mut c, quality);
            }
            *is_dirty = true;
        }
        if vc.bloom.quality == EffectQuality::Custom {
            let (label, value) = ("    Resolution Divisor:", vc.bloom.resolution_divisor);
            let (range, set) = (1..=MAX_RESOLUTION_DIVISOR, bloom::set_resolution_divisor);
            *is_dirty |= count_row(ui, (world, id), label, value, range, set);
        }
    }
}
