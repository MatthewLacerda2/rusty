//! src/editor/inspector/assets/shader.rs — the `.wgsl` inspector card (#352).
//!
//! Before this, `.wgsl` had no arm in `assets/mod.rs`'s dispatch at all and fell
//! into the generic "Unsupported file extension" fallback — the only way to see a
//! baked or hand-written shader was to build a whole scene around it. This card is
//! metadata-only; the actual "see it shaded" story is the Preview tab
//! (`inspector/preview`), which compiles this file into a one-off pipeline.

use std::fs;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("🎨 Shader: {}", filename));
    ui.add_space(5.0);
    draw_metadata_card(ui, path);
    ui.add_space(10.0);
    ui.colored_label(
        egui::Color32::GRAY,
        "Switch to the Preview tab to see this module shaded on a preview mesh.",
    );
}

fn draw_metadata_card(ui: &mut egui::Ui, path: &str) {
    egui::Frame::none()
        .fill(crate::editor::theme::from_ui(ui).bg_tier2)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Path: {}", path));
                let size_str = match fs::metadata(path) {
                    Ok(meta) => format!("{} bytes", meta.len()),
                    Err(_) => "Unknown size".to_string(),
                };
                ui.label(format!("Size: {}", size_str));
                ui.label("Type: WGSL Shader Module");
            });
        });
}
