//! src/editor/inspector/assets/lut.rs — the `.cube` colour-lookup card (#1441):
//! what the file holds (its size and domain), or why it is refused, the same
//! message the renderer logs. A strip `.png` is an image first and keeps the
//! image card.

use egui_phosphor::regular as icon;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);
    ui.heading(format!("{}  Color Lookup: {}", icon::SWATCHES, filename));
    ui.add_space(5.0);
    let summary = summary(ui.ctx(), path);
    super::metadata::draw_metadata_card(ui, path, |ui| {
        ui.label("Type: 3D LUT (.cube)");
        ui.label(&summary);
    });
    ui.add_space(10.0);
    ui.colored_label(
        crate::editor::theme::from_ui(ui).text_secondary,
        "Name it on a camera's Post Processing → Color Lookup, or Graphics.SetColorLookup.",
    );
}

/// The table's size and domain, or the parse error; read once per version of the
/// file (keyed by its modification time), not every frame.
fn summary(ctx: &egui::Context, path: &str) -> String {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let key = egui::Id::new(("lut_summary", path, modified));
    if let Some(cached) = ctx.data(|d| d.get_temp::<String>(key)) {
        return cached;
    }
    let text = match crate::asset::lut::load(Path::new(path)) {
        Ok(lut) => {
            let [lo, hi] = lut.domain;
            format!("Table: {0}³  Domain: {lo:?} – {hi:?}", lut.size)
        }
        Err(e) => format!("Refused: {e}"),
    };
    ctx.data_mut(|d| d.insert_temp(key, text.clone()));
    text
}
