use crate::core::scene::Scene;
use crate::editor::EditorUi;
use std::fs;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);
    let is_fbx = filename.to_lowercase().ends_with(".fbx");

    ui.heading(format!("📐 Model: {}", filename));
    ui.add_space(5.0);

    // File metadata card
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(20, 18, 30))
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
                ui.label(format!(
                    "Type: {} Model Asset",
                    if is_fbx { "FBX" } else { "OBJ" }
                ));
            });
        });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    // Model settings
    ui.heading("Model Import Settings");
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.label("Scale Factor:");
        ui.add(
            egui::DragValue::new(&mut editor.asset_model_scale)
                .speed(0.05)
                .clamp_range(0.01..=100.0),
        );
    });

    ui.checkbox(
        &mut editor.asset_model_import_normals,
        "Import Normals & Tangents",
    );

    ui.horizontal(|ui| {
        ui.label("Mesh Compression:");
        let mut compression = "Off";
        egui::ComboBox::from_id_source("MeshCompressionCombo")
            .selected_text(compression)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut compression, "Off", "Off");
                ui.selectable_value(&mut compression, "Low", "Low");
                ui.selectable_value(&mut compression, "Medium", "Medium");
                ui.selectable_value(&mut compression, "High", "High");
            });
    });

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    // Actions
    ui.heading("Instantiation");
    ui.add_space(5.0);

    if ui
        .add(egui::Button::new("➕ Instantiate into Scene").min_size(egui::Vec2::new(140.0, 30.0)))
        .clicked()
    {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Model")
            .to_string();
        let new_ent_id = scene.add_entity(format!("{}_Instance", stem));

        // Generate placeholder box geometry
        let scale = editor.asset_model_scale;
        let (v, idx) = crate::render::mesh::generate_box(scale, scale, scale);

        if let Some(mut ent) = scene.get_entity_mut(new_ent_id) {
            ent.mesh = Some(crate::core::scene::MeshComponent {
                primitive_type: if is_fbx {
                    "FBX".to_string()
                } else {
                    "OBJ".to_string()
                },
                vertices: v,
                indices: idx,
                is_dirty: crate::core::scene::DirtyFlag::new(true),
            });
            editor.is_dirty = true;

            // Unity-like convenience: auto-select the instantiated object and focus on it!
            editor.selected_entity_id = Some(new_ent_id);
            editor.selected_asset_path = None;
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
