//! The Screen Space Reflections section: the switch and, while on, the *Minimum
//! Smoothness* (#1736), the quality level and, at `Custom`, the march's knobs (#1522).

use super::{count_row, knob, knob_row, quality_row, toggle, Section};
use rusty_components::Ssr;
use rusty_core::settings::quality::EffectQuality;
use rusty_scene::authoring::rendering::post_processing::ssr;
use rusty_scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "ssr",
    group: None,
    order: 40,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
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
    let row = (
        "  Min Smoothness:",
        knob(ssr::EFFECT.knobs, "min_smoothness"),
    );
    *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
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
