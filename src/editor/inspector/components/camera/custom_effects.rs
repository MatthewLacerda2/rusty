//! The Custom Effects section of the Visual Correction card (#397): the volume's
//! ordered list of authored post-FX modules — add by name, reorder, remove.
//!
//! A thin client (#287): every edit builds the new list and writes it through
//! `authoring::visual_correction::set_custom_effects`, the op `Graphics.SetCustomEffects`
//! calls, so a name the op refuses is refused here too.

use egui_phosphor::regular as icon;

use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::VisualCorrectionComponent;

/// Draw the list and the add row; writes the edited list back through the op.
pub(super) fn draw_custom_effects(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &VisualCorrectionComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Custom Effects (after tonemap, in order)");
    let mut names = vc.custom_effects.clone();
    let mut edited = list_rows(ui, &mut names);

    // The pending name lives in egui memory, keyed by entity, until it is added.
    let key = egui::Id::new(("custom_effect_input", id));
    let mut input: String = ui.data_mut(|d| d.get_temp(key).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut input).hint_text("baked postfx name"));
        if ui.small_button(icon::PLUS).clicked() && !input.trim().is_empty() {
            names.push(input.trim().to_string());
            input.clear();
            edited = true;
        }
    });

    if edited {
        let result = world
            .visual_correction_mut(id)
            .map(|mut c| vc_ops::set_custom_effects(&mut c, names));
        match result {
            Some(Err(e)) => log::warn!("{e}"),
            Some(Ok(())) => *is_dirty = true,
            None => {}
        }
    }
    ui.data_mut(|d| d.insert_temp(key, input));
}

/// One row per effect with move-up / move-down / remove. Returns whether `names`
/// changed.
fn list_rows(ui: &mut egui::Ui, names: &mut Vec<String>) -> bool {
    let mut action = None;
    for (i, name) in names.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("    {}. {name}", i + 1));
            if ui.small_button(icon::ARROW_UP).clicked() && i > 0 {
                action = Some((i, Row::Up));
            }
            if ui.small_button(icon::ARROW_DOWN).clicked() && i + 1 < names.len() {
                action = Some((i, Row::Down));
            }
            if ui.small_button(icon::X).clicked() {
                action = Some((i, Row::Remove));
            }
        });
    }
    match action {
        Some((i, Row::Up)) => names.swap(i, i - 1),
        Some((i, Row::Down)) => names.swap(i, i + 1),
        Some((i, Row::Remove)) => drop(names.remove(i)),
        None => return false,
    }
    true
}

enum Row {
    Up,
    Down,
    Remove,
}
