//! src/editor/inspector/assets/shader.rs — the `.wgsl` inspector card (#352).
//!
//! Before this, `.wgsl` had no arm in `assets/mod.rs`'s dispatch at all and fell
//! into the generic "Unsupported file extension" fallback — the only way to see a
//! baked or hand-written shader was to build a whole scene around it. This card is
//! metadata-only; the actual "see it shaded" story is the Preview tab
//! (`inspector/preview`), which compiles this file into a one-off pipeline.

use egui_phosphor::regular as icon;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("{}  Shader: {}", icon::PALETTE, filename));
    ui.add_space(5.0);
    super::metadata::draw_metadata_card(ui, path, |ui| {
        ui.label("Type: WGSL Shader Module");
    });
    ui.add_space(10.0);
    ui.colored_label(
        crate::editor::theme::from_ui(ui).text_secondary,
        "Switch to the Preview tab to see this module shaded on a preview mesh.",
    );
}
