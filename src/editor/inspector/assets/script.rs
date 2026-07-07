use crate::editor::EditorUi;
use crate::scene::{Scene, ScriptComponent};
use crate::scripting::ConsoleLogs;
use std::fs;
use std::path::Path;

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    path: &str,
) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("📄 Script: {}", filename));
    ui.add_space(5.0);

    draw_metadata_card(ui, path);
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_code_editor(ui, editor, console, path, filename);
    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    draw_attach_to_entity(ui, editor, scene, console, path, filename);
}

/// File metadata card: path, on-disk size, and asset type.
fn draw_metadata_card(ui: &mut egui::Ui, path: &str) {
    egui::Frame::none()
        .fill(crate::editor::theme::from_ui(ui).bg_tier2)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Path: {}", path));
                let size_str = if let Ok(meta) = fs::metadata(path) {
                    format_size(meta.len())
                } else {
                    "Unknown size".to_string()
                };
                ui.label(format!("Size: {}", size_str));
                ui.label("Type: Lua Script");
            });
        });
}

/// Live script editor: an editable text area plus a button that writes the
/// buffer back to disk.
fn draw_code_editor(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    console: &mut ConsoleLogs,
    path: &str,
    filename: &str,
) {
    ui.heading("📝 Script Code Editor");
    ui.add_space(5.0);

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .id_source("ScriptEditScroll")
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut editor.asset_script_content)
                    .font(egui::FontId::monospace(12.0))
                    .desired_width(f32::INFINITY)
                    .desired_rows(12),
            );
        });

    ui.add_space(5.0);

    if ui
        .add(egui::Button::new("💾 Save Script Changes").min_size(egui::Vec2::new(140.0, 26.0)))
        .clicked()
    {
        match fs::write(path, &editor.asset_script_content) {
            Ok(_) => {
                console.info(format!("Saved script changes to {}", filename));
            }
            Err(e) => {
                console.error(format!("Failed to save script: {}", e));
            }
        }
    }
}

/// Entity picker that appends this script as a new `ScriptComponent` on the
/// chosen scene entity (#83: many scripts per entity).
fn draw_attach_to_entity(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    path: &str,
    filename: &str,
) {
    ui.heading("Attach to Scene Entity");
    ui.add_space(5.0);

    if scene.is_empty() {
        ui.colored_label(
            egui::Color32::GRAY,
            "No entities in scene to attach script to.",
        );
        return;
    }

    ui.label("Select an entity to attach this script:");
    let selected_ent_name = "Select Entity...".to_string();

    let mut clicked_entity_id = None;
    let mut clicked_entity_name = String::new();
    egui::ComboBox::from_id_source("AttachScriptToEntity")
        .selected_text(selected_ent_name)
        .show_ui(ui, |ui| {
            for id in scene.entity_ids() {
                let Some(name) = scene.world.name(id).map(|n| n.clone()) else {
                    continue;
                };
                if ui.selectable_label(false, &name).clicked() {
                    clicked_entity_id = Some(id);
                    clicked_entity_name = name;
                }
            }
        });

    if let Some(entity_id) = clicked_entity_id {
        if let Some(mut scripts) = scene.world.scripts_mut(entity_id) {
            // An entity can hold many scripts (#83); append rather than replace.
            scripts.push(ScriptComponent {
                path: path.to_string(),
                is_loaded: false,
                ..Default::default()
            });
            editor.is_dirty = true;
            console.info(format!(
                "Attached script {} to {}",
                filename, clicked_entity_name
            ));
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
