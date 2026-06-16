use crate::editor::EditorUi;
use crate::scene::Scene;
use std::fs;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_uppercase();

    ui.heading(format!("📐 Model: {}", filename));
    ui.add_space(5.0);

    // File metadata card
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
                ui.label(format!("Type: {} Model Asset", ext));
            });
        });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_sub_objects(ui, path);

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
        instantiate(editor, scene, path);
    }
}

/// List the addressable sub-objects the source file exposes (`path::<id>`). Reads
/// the `.meta` sidecar's cached map when present, else imports the file once and
/// writes the sidecar. This is the Content Browser surfacing a file's sub-objects.
fn draw_sub_objects(ui: &mut egui::Ui, path: &str) {
    let ids = match crate::asset::sidecar::load(Path::new(path)) {
        Ok(s) if !s.sub_objects.is_empty() => s.sub_objects,
        _ => crate::asset::import_and_sync_sidecar(Path::new(path))
            .map(|a| a.sub_mesh_ids())
            .unwrap_or_default(),
    };
    ui.heading(format!("Sub-Objects ({})", ids.len()));
    ui.add_space(5.0);
    if ids.is_empty() {
        ui.colored_label(egui::Color32::GRAY, "No importable sub-meshes found.");
    } else {
        for id in &ids {
            ui.label(format!(
                "• {}::{}",
                crate::editor::content_browser::file_name(path),
                id
            ));
        }
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);
}

/// Import the source file and instantiate each addressable sub-object as its own
/// entity carrying a path-based `"Asset"` mesh reference (`path::sub_object`). The
/// geometry is re-imported on every scene load — no GPU buffers are persisted.
fn instantiate(editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let asset = match crate::asset::import_and_sync_sidecar(Path::new(path)) {
        Ok(a) => a,
        Err(_) => return,
    };
    let mut last = None;
    for id in asset.sub_mesh_ids() {
        let reference = format!("{}{}{}", path, crate::asset::REF_SEPARATOR, id);
        let ent_id = scene.add_entity(id.clone());
        if let Some(mut ent) = scene.get_entity_mut(ent_id) {
            ent.mesh = Some(crate::scene::asset_mesh_component(&reference));
        }
        last = Some(ent_id);
    }
    if let Some(ent_id) = last {
        editor.is_dirty = true;
        editor.selected_entity_id = Some(ent_id);
        editor.selected_asset_path = None;
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
