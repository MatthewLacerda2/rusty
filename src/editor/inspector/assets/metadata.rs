//! The file metadata card every asset kind opens its inspector with (#720): the
//! path, the on-disk size, then the rows that describe this kind of asset.

use crate::editor::inspector::components::card::card_frame;

/// Draw the metadata card for the file at `path`. `rows` adds the kind-specific
/// lines under Path and Size (a type label, a scene's entity count, …).
pub(super) fn draw_metadata_card(ui: &mut egui::Ui, path: &str, rows: impl FnOnce(&mut egui::Ui)) {
    card_frame(&crate::editor::theme::from_ui(ui)).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(format!("Path: {}", path));
            let size_str = match std::fs::metadata(path) {
                Ok(meta) => format_size(meta.len()),
                Err(_) => "Unknown size".to_string(),
            };
            ui.label(format!("Size: {}", size_str));
            rows(ui);
        });
    });
}

/// A byte count as B, KB or MB, one decimal past bytes.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn format_size_picks_the_unit_at_each_boundary() {
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }
}
