//! The Text inspector card (#419): the string, fonts, size, colour, alignment,
//! wrapping, overflow, spacing, auto-size, rich text and raycast target, plus the
//! SDF effects (outline, shadow, glow) and the blend mode (#425). Widgets edit a snapshot; every write
//! routes through `scene::authoring::text`.

use egui_phosphor::regular as icon;

use super::look::{blend_row, color_row, drag_row};
use super::{combo, vec2_row};
use crate::components::{TextAlignment, TextComponent};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::text::{self as ops, FontSlot};

/// Text card. A THIN client (#287): widgets edit a snapshot and the changed
/// snapshot is written back field by field through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.text(id).map(|t| t.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::TEXT_T, "Text", Some(&mut remove), |ui| {
        changed |= ui.text_edit_multiline(&mut edit.text).changed();
        changed |= draw_font(ui, &mut edit);
        changed |= draw_paragraph(ui, &mut edit);
        ui.separator();
        changed |= draw_effects(ui, &mut edit);
        changed |= super::shader::row(ui, &mut edit.shader);
    });
    if changed {
        if let Some(mut t) = world.text_mut(id) {
            write_back(&mut t, edit);
        }
    }
    if remove {
        world.set_text(id, None);
    }
    *is_dirty |= remove || changed;
}

/// A labelled single-line path field; empty means "none".
fn path_row(ui: &mut egui::Ui, label: &str, path: &mut Option<String>) -> bool {
    let mut s = path.clone().unwrap_or_default();
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            ui.text_edit_singleline(&mut s).changed()
        })
        .inner;
    *path = Some(s);
    changed
}

/// Fonts, size and colour.
fn draw_font(ui: &mut egui::Ui, t: &mut TextComponent) -> bool {
    let mut changed = path_row(ui, "Font:", &mut t.font);
    changed |= path_row(ui, "Bold Font:", &mut t.font_bold);
    changed |= path_row(ui, "Italic Font:", &mut t.font_italic);
    changed |= drag_row(ui, "Font Size:", &mut t.font_size, 0.5);
    changed |= color_row(ui, "Color:", &mut t.color);
    changed
}

/// Alignment, wrapping, overflow, spacing, auto-size and the flags.
fn draw_paragraph(ui: &mut egui::Ui, t: &mut TextComponent) -> bool {
    let aligns = TextAlignment::ALL;
    let mut changed = combo(
        ui,
        "Alignment",
        &mut t.alignment,
        &aligns,
        ops::alignment_name,
    );
    changed |= ui.checkbox(&mut t.wrap, "Wrap").changed();
    changed |= combo(
        ui,
        "Overflow",
        &mut t.overflow,
        &ops::OVERFLOWS,
        ops::overflow_name,
    );
    changed |= drag_row(ui, "Line Spacing:", &mut t.line_spacing, 0.01);
    changed |= drag_row(ui, "Letter Spacing (em):", &mut t.letter_spacing, 0.005);
    changed |= ui.checkbox(&mut t.auto_size, "Auto Size").changed();
    if t.auto_size {
        changed |= drag_row(ui, "  Min:", &mut t.auto_size_min, 0.5);
        changed |= drag_row(ui, "  Max:", &mut t.auto_size_max, 0.5);
    }
    changed |= ui.checkbox(&mut t.rich_text, "Rich Text").changed();
    changed |= ui
        .checkbox(&mut t.raycast_target, "Raycast Target")
        .changed();
    changed
}

/// Outline, shadow and glow.
fn draw_effects(ui: &mut egui::Ui, t: &mut TextComponent) -> bool {
    let mut changed = drag_row(ui, "Outline (em):", &mut t.outline_width, 0.005);
    changed |= color_row(ui, "Outline Color:", &mut t.outline_color);
    if let Some(v) = vec2_row(ui, "Shadow Offset:", t.shadow_offset, 0.5) {
        t.shadow_offset = v;
        changed = true;
    }
    changed |= color_row(ui, "Shadow Color:", &mut t.shadow_color);
    changed |= drag_row(ui, "Glow (em):", &mut t.glow_size, 0.005);
    changed |= color_row(ui, "Glow Color:", &mut t.glow_color);
    changed |= blend_row(ui, &mut t.blend);
    changed
}

/// Write the edited snapshot back through the shared ops.
fn write_back(t: &mut TextComponent, e: TextComponent) {
    ops::set_text(t, e.text);
    ops::set_font(t, FontSlot::Regular, e.font);
    ops::set_font(t, FontSlot::Bold, e.font_bold);
    ops::set_font(t, FontSlot::Italic, e.font_italic);
    ops::set_font_size(t, e.font_size);
    ops::set_color(t, e.color);
    ops::set_alignment(t, e.alignment);
    ops::set_wrap(t, e.wrap);
    ops::set_overflow(t, e.overflow);
    ops::set_line_spacing(t, e.line_spacing);
    ops::set_letter_spacing(t, e.letter_spacing);
    ops::set_auto_size(t, e.auto_size, e.auto_size_min, e.auto_size_max);
    ops::set_rich_text(t, e.rich_text);
    ops::set_raycast_target(t, e.raycast_target);
    ops::set_outline(t, e.outline_width, e.outline_color);
    ops::set_shadow(t, e.shadow_offset, e.shadow_color);
    ops::set_glow(t, e.glow_size, e.glow_color);
    ops::set_blend(t, e.blend);
    super::shader::write_back(&mut t.shader, e.shader);
}
