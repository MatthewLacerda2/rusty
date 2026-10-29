//! The Screen Space Reflections section: the switch and, while on, the quality level
//! and, at `Custom`, the march's knobs (#1522).

use super::{knob, knob_row, quality_row, toggle, Section};
use crate::components::Ssr;
use crate::core::settings::quality::EffectQuality;
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
    if let Some(quality) = quality_row(ui, "SSR Quality", vc.ssr.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            ssr::set_quality(&mut c, quality);
        }
        *is_dirty = true;
    }
    if vc.ssr.quality != EffectQuality::Custom {
        return;
    }
    let knobs = ssr::EFFECT.knobs;
    let rows = [
        ("    Steps:", "steps", 1.0..=Ssr::MAX_STEPS as f32),
        (
            "    Step Length:",
            "step_length",
            Ssr::MIN_STEP_LENGTH..=Ssr::MAX_STEP_LENGTH,
        ),
    ];
    for (label, field, range) in rows {
        let row = (label, knob(knobs, field));
        *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, range);
    }
}
