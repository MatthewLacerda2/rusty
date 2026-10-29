//! The Screen Space Reflections section: the switch and, while on, the quality level
//! and, at `Custom`, the march's knobs (#1522).

use super::{count_row, knob, knob_row, quality_row, toggle, Section};
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
    let (steps, range) = (vc.ssr.steps, 1..=Ssr::MAX_STEPS);
    *is_dirty |= count_row(
        ui,
        (&mut *world, id),
        "    Steps:",
        steps,
        range,
        ssr::set_steps,
    );
    let row = ("    Step Length:", knob(ssr::EFFECT.knobs, "step_length"));
    let range = Ssr::MIN_STEP_LENGTH..=Ssr::MAX_STEP_LENGTH;
    *is_dirty |= knob_row(ui, (world, id), row, vc, range);
}
