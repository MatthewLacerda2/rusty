use crate::editor::EditorUi;
use crate::scene::Scene;
use std::fs;
use std::path::Path;

#[allow(clippy::too_many_lines)]
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

/// List the addressable sub-objects the source file exposes (`path::<id>`) and,
/// per sub-object, a "Mesh Collider" toggle whose trimesh/convex choice persists in
/// the `.meta` sidecar (#77). Reads the sidecar's cached map when present, else
/// imports the file once and writes the sidecar.
fn draw_sub_objects(ui: &mut egui::Ui, path: &str) {
    let mut settings = match crate::asset::sidecar::load(Path::new(path)) {
        Ok(s) if !s.sub_objects.is_empty() => s,
        _ => {
            let _ = crate::asset::import_and_sync_sidecar(Path::new(path));
            crate::asset::sidecar::load(Path::new(path)).unwrap_or_default()
        }
    };
    let ids = settings.sub_objects.clone();

    ui.heading(format!("Sub-Objects ({})", ids.len()));
    ui.add_space(5.0);
    if ids.is_empty() {
        ui.colored_label(egui::Color32::GRAY, "No importable sub-meshes found.");
    } else {
        let mut changed = false;
        for id in &ids {
            changed |= sub_object_row(ui, path, &mut settings.colliders, id);
        }
        if changed {
            let _ = crate::asset::sidecar::save(Path::new(path), &settings);
        }
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);
}

/// One sub-object row: its `file::id` label plus the mesh-collider toggle and
/// trimesh/convex selector. Returns `true` if the collider choice changed.
fn sub_object_row(
    ui: &mut egui::Ui,
    path: &str,
    colliders: &mut std::collections::BTreeMap<String, crate::asset::MeshColliderKind>,
    id: &str,
) -> bool {
    use crate::asset::MeshColliderKind;
    let mut generate = colliders.contains_key(id);
    let mut convex = colliders.get(id).map(|k| k.is_convex()).unwrap_or(false);

    ui.horizontal(|ui| {
        ui.label(format!(
            "• {}::{}",
            crate::editor::content_browser::file_name(path),
            id
        ));
        ui.checkbox(&mut generate, "Mesh Collider");
        if generate {
            egui::ComboBox::from_id_source(format!("mesh_collider_kind_{id}"))
                .selected_text(if convex { "Convex" } else { "Trimesh" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut convex, false, "Trimesh");
                    ui.selectable_value(&mut convex, true, "Convex");
                });
        }
    });

    let new = generate.then_some(if convex {
        MeshColliderKind::Convex
    } else {
        MeshColliderKind::TriMesh
    });
    if colliders.get(id).copied() == new {
        return false;
    }
    match new {
        Some(kind) => {
            colliders.insert(id.to_string(), kind);
        }
        None => {
            colliders.remove(id);
        }
    }
    true
}

/// Import the source file and instantiate each addressable sub-object as its own
/// entity carrying a path-based `"Asset"` mesh reference (`path::sub_object`). The
/// geometry is re-imported on every scene load — no GPU buffers are persisted.
fn instantiate(editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let asset = match crate::asset::import_and_sync_sidecar(Path::new(path)) {
        Ok(a) => a,
        Err(_) => return,
    };
    // The sidecar records which sub-objects get a baked mesh collider (#77).
    let settings = crate::asset::sidecar::load(Path::new(path)).unwrap_or_default();
    let mut last = None;
    let mut added_collider = false;
    for id in asset.sub_mesh_ids() {
        let reference = format!("{}{}{}", path, crate::asset::REF_SEPARATOR, id);
        let ent_id = scene.add_entity(id.clone());
        if let Some(mut ent) = scene.get_entity_mut(ent_id) {
            ent.mesh = Some(crate::scene::asset_mesh_component(&reference));
            if let Some(kind) = settings.colliders.get(&id) {
                if let Some((min, max)) = asset.sub_mesh(&id).and_then(|s| s.bounds()) {
                    ent.collider = Some(crate::components::ColliderComponent::from_mesh_bounds(
                        kind.is_convex(),
                        min,
                        max,
                    ));
                    added_collider = true;
                }
            }
        }
        last = Some(ent_id);
    }
    if added_collider {
        // Compute the new colliders' world AABBs for the editor overlays.
        scene.update_all_colliders();
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
