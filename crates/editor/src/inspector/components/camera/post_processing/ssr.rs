//! The Screen Space Reflections section: the switch and, while on, the *Minimum
//! Smoothness*, the *Algorithm*, the *Tracing* mode (with *Mixed*'s ray knobs), the quality
//! level and, at `Custom`, the march's knobs.

use super::{count_row, knob, knob_row, quality_row, toggle, Section};
use rusty_components::{Ssr, SsrAlgorithm, SsrTracing};
use rusty_core::settings::quality::{EffectQuality, MAX_RESOLUTION_DIVISOR};
use rusty_edit::authoring::rendering::post_processing::ssr;
use rusty_scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "ssr",
    group: None,
    order: 40,
    draw,
};

fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, vc: &PostProcessingComponent) {
    ui.add_space(3.0);
    ui.label("Screen Space Reflections (SSR)");
    let on = vc.ssr.active;
    toggle(ui, (&mut *world, id), "  SSR Active", on, ssr::set_active);
    if !on {
        return;
    }
    let row = (
        "  Min Smoothness:",
        knob(ssr::EFFECT.knobs, "min_smoothness"),
    );
    knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
    algorithm_row(ui, &mut *world, id, vc.ssr.algorithm);
    tracing_rows(ui, &mut *world, id, vc);
    if let Some(quality) = quality_row(ui, "SSR Quality", vc.ssr.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            ssr::set_quality(&mut c, quality);
        }
    }
    if vc.ssr.quality != EffectQuality::Custom {
        return;
    }
    let (steps, range) = (vc.ssr.steps, 1..=Ssr::MAX_STEPS);
    count_row(
        ui,
        (&mut *world, id),
        "    Steps:",
        steps,
        range,
        ssr::set_steps,
    );
    let row = ("    Step Length:", knob(ssr::EFFECT.knobs, "step_length"));
    let range = Ssr::MIN_STEP_LENGTH..=Ssr::MAX_STEP_LENGTH;
    knob_row(ui, (&mut *world, id), row, vc, range);
    if vc.ssr.tracing == SsrTracing::Mixed {
        let divisor = vc.ssr.trace_resolution_divisor;
        let (label, set) = (
            "    Trace Resolution Divisor:",
            ssr::set_trace_resolution_divisor,
        );
        count_row(
            ui,
            (world, id),
            label,
            divisor,
            1..=MAX_RESOLUTION_DIVISOR,
            set,
        );
    }
}

/// The *Algorithm* selector.
fn algorithm_row(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, now: SsrAlgorithm) {
    let mut algorithm = now;
    ui.horizontal(|ui| {
        ui.label("  Algorithm:");
        egui::ComboBox::from_id_salt("ssr_algorithm")
            .selected_text(algorithm.name())
            .show_ui(ui, |ui| {
                for value in SsrAlgorithm::ALL {
                    let picked = ui.selectable_value(&mut algorithm, value, value.name());
                    if picked.changed() {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            ssr::set_algorithm(&mut c, algorithm);
                        }
                    }
                }
            });
    });
}

/// The *Tracing* selector and, under *Mixed*, its ray's cutoffs.
fn tracing_rows(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
) {
    let mut tracing = vc.ssr.tracing;
    ui.horizontal(|ui| {
        ui.label("  Tracing:");
        egui::ComboBox::from_id_salt("ssr_tracing")
            .selected_text(tracing.name())
            .show_ui(ui, |ui| {
                for value in SsrTracing::ALL {
                    let picked = ui.selectable_value(&mut tracing, value, value.name());
                    if picked.changed() {
                        if let Some(mut c) = world.post_processing_mut(id) {
                            ssr::set_tracing(&mut c, tracing);
                        }
                    }
                }
            });
    });
    if vc.ssr.tracing != SsrTracing::Mixed {
        return;
    }
    let row = (
        "    Ray Min Smoothness:",
        knob(ssr::EFFECT.knobs, "ray_min_smoothness"),
    );
    knob_row(ui, (&mut *world, id), row, vc, 0.0..=1.0);
    let row = ("    Ray Length:", knob(ssr::EFFECT.knobs, "ray_length"));
    let range = Ssr::MIN_RAY_LENGTH..=Ssr::MAX_RAY_LENGTH;
    knob_row(ui, (world, id), row, vc, range);
}
