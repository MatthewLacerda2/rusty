//! The custom-shader row the Image, Shape and Text cards share (#427): the name of the
//! baked ui shader the graphic draws with (empty: the standard UI shader). Edits go
//! through `scene::authoring::ui_shader`, like `UI.SetShader`; runtime params are
//! driven from scripts (`UI.SetShaderParam`).

use crate::components::UiShader;
use crate::scene::authoring::ui_shader as ops;

/// Edit the snapshot's shader name; returns whether it changed.
pub fn row(ui: &mut egui::Ui, slot: &mut Option<UiShader>) -> bool {
    let mut name = slot.as_ref().map(|s| s.name.clone()).unwrap_or_default();
    let changed = ui
        .horizontal(|ui| {
            ui.label("UI Shader:").on_hover_text(
                "A shader baked with Shader.Bake pass \"ui\"; empty draws the standard one",
            );
            ui.text_edit_singleline(&mut name).changed()
        })
        .inner;
    if changed {
        ops::set_shader(slot, Some(name));
    }
    changed
}

/// Write the snapshot's shader back: the same name keeps the live param values.
pub fn write_back(live: &mut Option<UiShader>, edit: Option<UiShader>) {
    ops::set_shader(live, edit.map(|s| s.name));
}
