//! The Screen Space Indirect Lighting section: the switch and, while on,
//! its intensity and radius, its quality level and, at `Custom`, its sample count
//! and resolution divisor. It sits beside SSAO, the other indirect-light section.

use super::{count_row, knob, knob_row, quality_row, toggle, Section};
use rusty_components::Ssil;
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use rusty_edit::authoring::rendering::post_processing::ssil;
use rusty_scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "ssil",
    group: None,
    order: 52,
    draw,
};

fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, vc: &PostProcessingComponent) {
    ui.add_space(3.0);
    ui.label("Screen Space Indirect Lighting (SSIL)");
    let on = vc.ssil.active;
    let cx = (&mut *world, id);
    toggle(ui, cx, "  SSIL Active", on, ssil::set_active);
    if !on {
        return;
    }
    let knobs = ssil::EFFECT.knobs;
    let row = ("    Intensity:", knob(knobs, "intensity"));
    knob_row(ui, (&mut *world, id), row, vc, 0.0..=Ssil::MAX_INTENSITY);
    let row = ("    Radius:", knob(knobs, "radius"));
    let range = Ssil::MIN_RADIUS..=Ssil::MAX_RADIUS;
    knob_row(ui, (&mut *world, id), row, vc, range);
    if let Some(quality) = quality_row(ui, "SSIL Quality", vc.ssil.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            ssil::set_quality(&mut c, quality);
        }
    }
    if vc.ssil.quality != EffectQuality::Custom {
        return;
    }
    let samples = (vc.ssil.samples, 1..=Ssil::MAX_SAMPLES);
    let cx = (&mut *world, id);
    count_row(
        ui,
        cx,
        "    Samples:",
        samples.0,
        samples.1,
        ssil::set_samples,
    );
    let divisor = (vc.ssil.resolution_divisor, 1..=MAX_RESOLUTION_DIVISOR);
    let (label, set) = ("    Resolution Divisor:", ssil::set_resolution_divisor);
    count_row(ui, (world, id), label, divisor.0, divisor.1, set);
}
