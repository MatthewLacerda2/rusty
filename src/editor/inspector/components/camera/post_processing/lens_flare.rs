//! The Lens Flare section (#1443): its switch and, while on, its strengths, the
//! ghost count, the tint and the flare's chromatic aberration.

use super::{knob, knob_row, toggle, Section};
use crate::components::rendering::post_processing::LensFlare;
use crate::scene::authoring::rendering::post_processing::lens_flare;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "lens_flare",
    group: None,
    order: 11,
    draw,
};

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let on = vc.lens_flare.active;
    toggle(
        ui,
        (&mut *world, id),
        "  Lens Flare",
        on,
        lens_flare::set_active,
        is_dirty,
    );
    if !on {
        return;
    }
    ghost_count(ui, (&mut *world, id), vc, is_dirty);
    let knobs = lens_flare::EFFECT.knobs;
    let rows = [
        ("    Intensity:", "intensity", 5.0),
        ("    Ghost Intensity:", "ghost_intensity", 5.0),
        ("    Halo Intensity:", "halo_intensity", 5.0),
        ("    Streak Intensity:", "streak_intensity", 5.0),
        ("    Chromatic Aberration:", "chromatic_aberration", 1.0),
    ];
    for (label, field, max) in rows {
        let row = (label, knob(knobs, field));
        *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, 0.0..=max);
    }
    tint(ui, (world, id), vc, is_dirty);
}

/// The ghost count's slider, a whole number.
fn ghost_count(
    ui: &mut egui::Ui,
    (world, id): (&mut crate::ecs::World, u32),
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let mut count = vc.lens_flare.ghost_count;
    let changed = ui
        .horizontal(|ui| {
            ui.label("    Ghosts:");
            let range = 0..=LensFlare::MAX_GHOSTS;
            ui.add(crate::editor::widgets::slider(&mut count, range))
                .changed()
        })
        .inner;
    if changed {
        if let Some(mut c) = world.post_processing_mut(id) {
            lens_flare::set_ghost_count(&mut c, count);
        }
        *is_dirty = true;
    }
}

/// The tint's colour button.
fn tint(
    ui: &mut egui::Ui,
    (world, id): (&mut crate::ecs::World, u32),
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let mut tint = vc.lens_flare.tint;
    let changed = ui
        .horizontal(|ui| {
            ui.label("    Tint:");
            ui.color_edit_button_rgb(&mut tint).changed()
        })
        .inner;
    if changed {
        if let Some(mut c) = world.post_processing_mut(id) {
            lens_flare::set_tint(&mut c, tint);
        }
        *is_dirty = true;
    }
}
