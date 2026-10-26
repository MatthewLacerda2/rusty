//! The Lens Effects section of the Post Processing card (#1439): Vignette,
//! Chromatic Aberration and Film Grain, each a checkbox and, when on, its
//! parameters. Split out of `post_processing` to keep it under the size cap.
//!
//! A THIN client (#287): each widget reads the snapshot and writes through the
//! shared `authoring::rendering::post_processing` op `Graphics.*` calls too.

use super::post_processing::slider_row;
use crate::components::Vignette;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::PostProcessingComponent;

/// The three effects, in the order the composite runs them.
pub(super) fn draw_lens(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Lens Effects");
    draw_aberration(ui, world, id, vc, is_dirty);
    draw_vignette(ui, world, id, vc, is_dirty);
    draw_grain(ui, world, id, vc, is_dirty);
}

/// Chromatic Aberration: on/off and intensity.
fn draw_aberration(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let ca = vc.chromatic_aberration;
    toggle(
        ui,
        (&mut *world, id),
        "  Chromatic Aberration",
        ca.active,
        vc_ops::lens::chromatic_aberration::set_active,
        is_dirty,
    );
    if ca.active {
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Intensity:",
            ca.intensity,
            0.0..=1.0,
            vc_ops::lens::chromatic_aberration::set_intensity,
        );
    }
}

/// Vignette: on/off, intensity, smoothness, colour and centre.
fn draw_vignette(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let v = vc.vignette;
    toggle(
        ui,
        (&mut *world, id),
        "  Vignette",
        v.active,
        vc_ops::lens::vignette::set_active,
        is_dirty,
    );
    if v.active {
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Intensity:",
            v.intensity,
            0.0..=1.0,
            vc_ops::lens::vignette::set_intensity,
        );
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Smoothness:",
            v.smoothness,
            Vignette::MIN_SMOOTHNESS..=1.0,
            vc_ops::lens::vignette::set_smoothness,
        );
        draw_vignette_look(ui, world, id, v, is_dirty);
    }
}

/// Film Grain: on/off, intensity and response.
fn draw_grain(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let grain = vc.film_grain;
    toggle(
        ui,
        (&mut *world, id),
        "  Film Grain",
        grain.active,
        vc_ops::lens::film_grain::set_active,
        is_dirty,
    );
    if grain.active {
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Intensity:",
            grain.intensity,
            0.0..=1.0,
            vc_ops::lens::film_grain::set_intensity,
        );
        *is_dirty |= slider_row(
            ui,
            (&mut *world, id),
            "    Response:",
            grain.response,
            0.0..=1.0,
            vc_ops::lens::film_grain::set_response,
        );
    }
}

/// One effect's `active` checkbox.
fn toggle(
    ui: &mut egui::Ui,
    (world, id): (&mut crate::ecs::World, u32),
    label: &str,
    mut on: bool,
    op: fn(&mut PostProcessingComponent, bool),
    is_dirty: &mut bool,
) {
    if ui.checkbox(&mut on, label).changed() {
        if let Some(mut c) = world.post_processing_mut(id) {
            op(&mut c, on);
        }
        *is_dirty = true;
    }
}

/// The vignette's colour and centre (one row per axis: two sliders overflow the
/// inspector's width).
fn draw_vignette_look(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    mut v: Vignette,
    is_dirty: &mut bool,
) {
    let changed = ui
        .horizontal(|ui| {
            ui.label("    Color:");
            ui.color_edit_button_rgb(&mut v.color).changed()
        })
        .inner;
    if changed {
        if let Some(mut c) = world.post_processing_mut(id) {
            vc_ops::lens::vignette::set_color(&mut c, v.color);
        }
        *is_dirty = true;
    }
    let [x, y] = v.center;
    *is_dirty |= slider_row(
        ui,
        (&mut *world, id),
        "    Center X:",
        x,
        0.0..=1.0,
        set_center_x,
    );
    *is_dirty |= slider_row(ui, (world, id), "    Center Y:", y, 0.0..=1.0, set_center_y);
}

// One axis of the centre, the other kept.

fn set_center_x(vc: &mut PostProcessingComponent, x: f32) {
    vc_ops::lens::vignette::set_center(vc, [x, vc.vignette.center[1]]);
}

fn set_center_y(vc: &mut PostProcessingComponent, y: f32) {
    vc_ops::lens::vignette::set_center(vc, [vc.vignette.center[0], y]);
}
