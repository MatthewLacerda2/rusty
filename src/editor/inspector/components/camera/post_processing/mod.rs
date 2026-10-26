//! src/editor/inspector/components/camera/post_processing/ — the Post Processing
//! inspector card: one section per post effect (#1467), each in its own file
//! (grading and lens effects in their folders), plus the volume's shadow and SSAO
//! sections (`volume`). [`SECTIONS`] lists them; the card draws them by the display
//! key each declares, never by list position, under the group heading it names.
//!
//! A THIN client (#287): each widget reads the field from a snapshot and routes its
//! write through the shared `authoring::rendering::post_processing` op (which the
//! `Graphics.*` Lua surface calls too), never mutating the component directly.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::{component_card, remove_component};
use crate::scene::authoring::rendering::post_processing::{self as vc_ops, Knob};
use crate::scene::authoring::ComponentKind;
use crate::scene::PostProcessingComponent;

// sorted: one module per section (or family of sections); none reads another
mod bloom;
mod custom_effects;
mod depth_of_field;
#[cfg(test)]
mod dummy;
mod exposure;
mod grading;
mod lens;
mod ssr;
mod volume;

#[cfg(test)]
mod slider_tests;

pub use grading::commit_abandoned_lut;

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

/// How a section draws: on `id`'s volume, from the snapshot `vc`, marking the scene
/// dirty on a write.
pub(crate) type Draw =
    fn(&mut egui::Ui, &mut crate::ecs::World, u32, &PostProcessingComponent, &mut bool);

/// One section of the card.
pub(crate) struct Section {
    /// The effect it edits, as the volume names its field (or `shadows`, `ssao`).
    pub effect: &'static str,
    /// The heading it sits under, drawn once above the first section of a run.
    pub group: Option<&'static str>,
    /// Where it sits in the card: lower first.
    pub order: u16,
    pub draw: Draw,
}

/// The grading sections' heading.
const COLOR_CORRECTION: Option<&str> = Some("Color Correction");
/// The lens sections' heading.
const LENS_EFFECTS: Option<&str> = Some("Lens Effects");

/// Every section of the card, by effect.
pub(crate) const SECTIONS: &[Section] = &[
    // sorted: one row per section; the card orders them by `order`
    bloom::SECTION,
    chromatic_aberration::SECTION,
    color_adjustments::SECTION,
    color_lookup::SECTION,
    custom_effects::SECTION,
    depth_of_field::SECTION,
    #[cfg(test)]
    dummy::SECTION,
    exposure::SECTION,
    film_grain::SECTION,
    lift_gamma_gain::SECTION,
    ssr::SECTION,
    tonemapping::SECTION,
    vignette::SECTION,
    volume::SHADOWS,
    volume::SSAO,
    white_balance::SECTION,
];

/// Post Processing Component panel.
pub fn draw_post_processing(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(vc) = world.post_processing(id).map(|vc| vc.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::SPARKLE,
        "Post Processing",
        Some(&mut remove),
        |ui| {
            let mut active = vc.active;
            if ui.checkbox(&mut active, "Enable Post Processing").changed() {
                if let Some(mut c) = world.post_processing_mut(id) {
                    vc_ops::volume::set_active(&mut c, active);
                }
                *is_dirty = true;
            }
            let mut group = None;
            for section in ordered() {
                if section.group.is_some() && section.group != group {
                    ui.add_space(3.0);
                    ui.label(section.group.unwrap_or_default());
                }
                group = section.group;
                // Each section's widget ids apart: two sections' combo boxes never collide.
                ui.push_id(section.effect, |ui| {
                    (section.draw)(ui, world, id, &vc, is_dirty)
                });
            }
        },
    );
    if remove {
        remove_component(world, id, ComponentKind::PostProcessing, is_dirty);
    }
}

/// [`SECTIONS`] in the card's order.
fn ordered() -> Vec<&'static Section> {
    let mut sections: Vec<&Section> = SECTIONS.iter().collect();
    sections.sort_by_key(|s| s.order);
    sections
}

/// A labelled slider row that routes its write through a shared vc op (#287).
/// Returns whether the value changed; `cx` is the (world, entity-id) pair.
pub(super) fn slider_row(
    ui: &mut egui::Ui,
    cx: (&mut crate::ecs::World, u32),
    label: &str,
    mut value: f32,
    range: std::ops::RangeInclusive<f32>,
    op: fn(&mut PostProcessingComponent, f32),
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let changed = ui
            .add(crate::editor::widgets::slider(&mut value, range))
            .changed();
        if changed {
            if let Some(mut c) = cx.0.post_processing_mut(cx.1) {
                op(&mut c, value);
            }
        }
        changed
    })
    .inner
}

/// [`slider_row`] for `knob`, read from `vc`.
pub(super) fn knob_row(
    ui: &mut egui::Ui,
    cx: (&mut crate::ecs::World, u32),
    (label, knob): (&str, &Knob),
    vc: &PostProcessingComponent,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    slider_row(ui, cx, label, (knob.get)(vc), range, knob.set)
}

/// One `active` checkbox, written through `op`.
pub(super) fn toggle(
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

/// The knob at `field` of `knobs`, one a section names.
pub(super) fn knob(knobs: &'static [Knob], field: &str) -> &'static Knob {
    let knob = knobs.iter().find(|k| k.field == field);
    knob.unwrap_or_else(|| panic!("no knob `{field}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_its_own_place_in_the_card() {
        let mut orders: Vec<u16> = SECTIONS.iter().map(|s| s.order).collect();
        orders.sort_unstable();
        orders.dedup();
        assert_eq!(orders.len(), SECTIONS.len(), "two sections share an order");
    }
}
