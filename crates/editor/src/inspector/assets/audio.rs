use egui_phosphor::regular as icon;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("{}  Audio: {}", icon::MUSIC_NOTES, filename));
    ui.add_space(5.0);

    super::metadata::draw_metadata_card(ui, path, |ui| {
        ui.label("Type: Audio Clip");
    });
}
