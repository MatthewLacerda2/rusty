use crate::editor::EditorUi;
use crate::scene::{MaterialComponent, Scene};
use std::fs;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("🖼️ Image: {}", filename));
    ui.add_space(5.0);

    draw_metadata_card(ui, path);
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_import_settings(ui, editor);
    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    draw_apply_to_entity(ui, editor, scene, path);
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
                ui.label("Type: Texture Asset");
            });
        });
}

/// Texture import settings: wrap mode, filter mode, and mipmap generation.
fn draw_import_settings(ui: &mut egui::Ui, editor: &mut EditorUi) {
    ui.heading("Texture Import Settings");
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.label("Wrap Mode:");
        egui::ComboBox::from_id_source("ImgWrapMode")
            .selected_text(&editor.asset_image_wrap)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut editor.asset_image_wrap, "Repeat".to_string(), "Repeat");
                ui.selectable_value(
                    &mut editor.asset_image_wrap,
                    "Clamp".to_string(),
                    "Clamp to Edge",
                );
                ui.selectable_value(&mut editor.asset_image_wrap, "Mirror".to_string(), "Mirror");
            });
    });

    ui.horizontal(|ui| {
        ui.label("Filter Mode:");
        egui::ComboBox::from_id_source("ImgFilterMode")
            .selected_text(&editor.asset_image_filter)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut editor.asset_image_filter,
                    "Linear".to_string(),
                    "Linear (Smooth)",
                );
                ui.selectable_value(
                    &mut editor.asset_image_filter,
                    "Nearest".to_string(),
                    "Nearest (Pixelated)",
                );
            });
    });

    ui.checkbox(&mut editor.asset_image_mipmaps, "Generate Mipmaps");
}

/// Entity picker that applies this texture to the chosen scene entity.
fn draw_apply_to_entity(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    ui.heading("Apply to Scene Entity");
    ui.add_space(5.0);

    if scene.is_empty() {
        ui.colored_label(
            egui::Color32::GRAY,
            "No entities in scene to apply texture to.",
        );
        return;
    }

    ui.label("Choose an entity to apply this texture:");
    let selected_ent_name = "Select Entity...".to_string();

    let mut clicked_entity_id = None;
    egui::ComboBox::from_id_source("ApplyTextureToEntity")
        .selected_text(selected_ent_name)
        .show_ui(ui, |ui| {
            for entity in scene.iter() {
                if ui.selectable_label(false, &entity.name).clicked() {
                    clicked_entity_id = Some(entity.id);
                }
            }
        });

    if let Some(entity_id) = clicked_entity_id {
        apply_albedo_to_entity(scene, entity_id, path);
        editor.is_dirty = true;
    }
}

/// Attach (or update) a material on `entity_id` whose albedo is `path`, creating a
/// library material under a per-entity key if the entity has none yet. Reuses the
/// entity's existing referenced material when present so its other params survive.
fn apply_albedo_to_entity(scene: &mut Scene, entity_id: u32, path: &str) {
    let key = {
        let Some(mut ent) = scene.get_entity_mut(entity_id) else {
            return;
        };
        if ent.material.is_none() {
            ent.material = Some(MaterialComponent {
                material: format!("entity_{entity_id}_material"),
            });
        }
        ent.material.as_ref().unwrap().material.clone()
    };
    let mat = scene.materials.entry(key).or_default();
    mat.base_color_map = Some(path.to_string());
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
