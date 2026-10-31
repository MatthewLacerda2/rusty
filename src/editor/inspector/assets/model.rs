use crate::editor::widgets::DragRange;
use crate::editor::EditorUi;
use crate::scene::Scene;
use egui_phosphor::regular as icon;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("{}  Model: {}", icon::CUBE, filename));
    ui.add_space(5.0);

    draw_metadata(ui, path);
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_sub_objects(ui, path);

    ui.heading("Model Import Settings");
    ui.add_space(5.0);
    draw_scale_factor(ui, editor, scene, path);
    draw_lightmap_uvs(ui, scene, path);

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    // Actions
    ui.heading("Instantiation");
    ui.add_space(5.0);

    if ui
        .add(
            egui::Button::new(format!("{}  Instantiate into Scene", icon::PLUS))
                .min_size(egui::Vec2::new(140.0, 24.0)),
        )
        .clicked()
    {
        instantiate(editor, scene, path);
    }
}

/// File metadata card: path, on-disk size, and the asset type derived from the
/// file extension.
fn draw_metadata(ui: &mut egui::Ui, path: &str) {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_uppercase();
    super::metadata::draw_metadata_card(ui, path, |ui| {
        ui.label(format!("Type: {} Model Asset", ext));
    });
}

/// Scale Factor (#1398): Unity's model-import drag, kept in the `.meta` sidecar.
/// A finished edit goes through the verb `Assets.SetModelImportSettings` uses,
/// re-importing the model's meshes (and rigs) in the scene. Mid-drag the value is
/// held in egui's memory, not the sidecar: the verb changes nothing for a factor
/// the sidecar already holds, so the sidecar must still hold the old one when the
/// drag is released. A commit that re-imported a mesh marks the scene dirty: its
/// mesh colliders' boxes and LODGroup sizes moved.
fn draw_scale_factor(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let held = egui::Id::new(("model_scale_factor", path));
    let stored = crate::asset::sidecar::load(Path::new(path))
        .unwrap_or_default()
        .scale;
    let mut scale = ui.data(|d| d.get_temp::<f32>(held)).unwrap_or(stored);
    let r = ui
        .horizontal(|ui| {
            ui.label("Scale Factor:");
            ui.add(
                egui::DragValue::new(&mut scale)
                    .speed(0.01)
                    .ranged(crate::asset::scale_factor::MIN_SCALE..=f32::MAX),
            )
        })
        .inner;
    if r.drag_stopped() || (r.changed() && !r.dragged()) {
        ui.data_mut(|d| d.remove::<f32>(held));
        let set = crate::scene::asset_instance::set_model_scale_factor(scene, path, scale);
        editor.is_dirty |= set.is_ok_and(|n| n > 0);
    } else if r.dragged() {
        ui.data_mut(|d| d.insert_temp(held, scale));
    } else {
        ui.data_mut(|d| d.remove::<f32>(held));
    }
}

/// Generate Lightmap UVs (#831): Unity's checkbox and its two knobs, kept in the
/// `.meta` sidecar. A finished edit (a click, a released drag, a typed value) goes
/// through the verb `Assets.SetLightmapUVSettings` uses, re-importing the model's
/// meshes in the scene; mid-drag only the sidecar is written, so dragging a knob
/// over a big level never re-imports it every frame.
fn draw_lightmap_uvs(ui: &mut egui::Ui, scene: &mut Scene, path: &str) {
    let source = Path::new(path);
    let stored = crate::asset::sidecar::load(source)
        .unwrap_or_default()
        .lightmap_uvs;
    let mut s = stored;
    let mut commit = ui
        .checkbox(&mut s.generate, "Generate Lightmap UVs")
        .changed();
    if s.generate {
        let mut knob = |ui: &mut egui::Ui, label: &str, value: &mut f32, max: f32| {
            ui.horizontal(|ui| {
                ui.label(label);
                let r = ui.add(egui::DragValue::new(value).speed(0.5).ranged(0.0..=max));
                commit |= r.drag_stopped() || (r.changed() && !r.dragged());
            });
        };
        knob(ui, "Hard Angle:", &mut s.hard_angle, 180.0);
        knob(ui, "Pack Margin:", &mut s.pack_margin, 64.0);
    }
    if commit {
        let _ = crate::scene::asset_instance::set_lightmap_uv_settings(scene, path, s);
    } else if s != stored {
        let _ = crate::asset::sidecar::set_lightmap_uvs(source, s);
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
        ui.colored_label(
            crate::editor::theme::from_ui(ui).text_secondary,
            "No importable sub-meshes found.",
        );
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
        let file = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path);
        ui.label(format!("• {file}::{id}"));
        ui.checkbox(&mut generate, "Mesh Collider");
        if generate {
            egui::ComboBox::from_id_salt(format!("mesh_collider_kind_{id}"))
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

/// Import the source file and instantiate it as Unity's model prefab (#1083): a
/// root named after the file and one child per node, each mesh node carrying a
/// path-based `"Asset"` mesh reference (`path::sub_object`) — a `_LOD<n>` set as one
/// LODGroup entity (#472). Routes through the shared `authoring::instantiate_model`,
/// the verb `Scene.Instantiate` of a bare model path drives too, so editor and
/// API never drift. The
/// geometry is re-imported on every scene load; no GPU buffers are persisted.
fn instantiate(editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let asset = match crate::asset::import_and_sync_sidecar(Path::new(path)) {
        Ok(a) => a,
        Err(_) => return,
    };
    let root = crate::scene::authoring::instantiate_model(scene, path, &asset, glam::Vec3::ZERO);
    editor.is_dirty = true;
    editor.selected_entity_id = Some(root);
    editor.selected_asset_path = None;
}

#[cfg(test)]
#[path = "model_material_tests.rs"]
mod model_material_tests;
