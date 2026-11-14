use crate::widgets::DragRange;
use crate::EditorUi;
use egui_phosphor::regular as icon;
use rusty_asset::{MeshColliderKind, SubObjectCollider};
use rusty_scene::Scene;
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
    let stored = rusty_asset::sidecar::load(Path::new(path))
        .unwrap_or_default()
        .scale;
    let mut scale = ui.data(|d| d.get_temp::<f32>(held)).unwrap_or(stored);
    let r = ui
        .horizontal(|ui| {
            ui.label("Scale Factor:");
            ui.add(
                egui::DragValue::new(&mut scale)
                    .speed(0.01)
                    .ranged(rusty_asset::scale_factor::MIN_SCALE..=f32::MAX),
            )
        })
        .inner;
    if r.drag_stopped() || (r.changed() && !r.dragged()) {
        ui.data_mut(|d| d.remove::<f32>(held));
        let set = rusty_scene::asset_instance::set_model_scale_factor(scene, path, scale);
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
    let stored = rusty_asset::sidecar::load(source)
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
        let _ = rusty_scene::asset_instance::set_lightmap_uv_settings(scene, path, s);
    } else if s != stored {
        let _ = rusty_asset::sidecar::set_lightmap_uvs(source, s);
    }
}

/// List the addressable sub-objects the source file exposes (`path::<id>`) and,
/// per sub-object, a "Mesh Collider" toggle whose trimesh/convex choice and collision
/// mesh persist in the `.meta` sidecar (#77, #1960). Reads the sidecar's cached map when present, else
/// imports the file once and writes the sidecar.
fn draw_sub_objects(ui: &mut egui::Ui, path: &str) {
    let mut settings = match rusty_asset::sidecar::load(Path::new(path)) {
        Ok(s) if !s.sub_objects.is_empty() => s,
        _ => {
            let _ = rusty_asset::import_and_sync_sidecar(Path::new(path));
            rusty_asset::sidecar::load(Path::new(path)).unwrap_or_default()
        }
    };
    let ids = settings.sub_objects.clone();

    ui.heading(format!("Sub-Objects ({})", ids.len()));
    ui.add_space(5.0);
    if ids.is_empty() {
        ui.colored_label(
            crate::theme::from_ui(ui).text_secondary,
            "No importable sub-meshes found.",
        );
    } else {
        let mut changed = false;
        for id in &ids {
            changed |= sub_object_row(ui, path, &mut settings.colliders, &ids, id);
        }
        if changed {
            let _ = rusty_asset::sidecar::save(Path::new(path), &settings);
        }
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);
}

/// One sub-object row: its `file::id` label plus the mesh-collider toggle, the
/// trimesh/convex selector and the collision mesh it builds from (#1960: its own,
/// or another sub-object of the file). Returns `true` if the collider choice changed.
fn sub_object_row(
    ui: &mut egui::Ui,
    path: &str,
    colliders: &mut std::collections::BTreeMap<String, SubObjectCollider>,
    ids: &[String],
    id: &str,
) -> bool {
    let stored = colliders.get(id).cloned();
    let mut generate = stored.is_some();
    let mut convex = stored.as_ref().is_some_and(|c| c.kind.is_convex());
    let mut mesh = stored.as_ref().and_then(|c| c.mesh.clone());

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
    // Its own line: the row above is already as wide as a narrow inspector.
    if generate {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            ui.label("Collision mesh");
            collision_mesh_combo(ui, &mut mesh, colliders, ids, id);
        });
    }

    let kind = if convex {
        MeshColliderKind::Convex
    } else {
        MeshColliderKind::TriMesh
    };
    let new = generate.then_some(SubObjectCollider { kind, mesh });
    if stored == new {
        return false;
    }
    match new {
        Some(entry) => {
            colliders.insert(id.to_string(), entry);
        }
        None => {
            colliders.remove(id);
        }
    }
    true
}

/// The collision-mesh combo of `id`'s row (#1960): its own mesh, or another
/// sub-object of the file, which a whole-model spawn then never draws. A sub-object
/// whose own collision mesh leads back to `id` is not offered: the two would name
/// each other and neither be drawn (`sidecar::closes_loop`).
fn collision_mesh_combo(
    ui: &mut egui::Ui,
    mesh: &mut Option<String>,
    colliders: &std::collections::BTreeMap<String, SubObjectCollider>,
    ids: &[String],
    id: &str,
) {
    let offered = |o: &&String| *o != id && !rusty_asset::sidecar::closes_loop(colliders, id, o);
    egui::ComboBox::from_id_salt(format!("mesh_collider_mesh_{id}"))
        .selected_text(mesh.as_deref().unwrap_or("Own mesh"))
        .show_ui(ui, |ui| {
            ui.selectable_value(mesh, None, "Own mesh");
            for other in ids.iter().filter(offered) {
                ui.selectable_value(mesh, Some(other.clone()), other);
            }
        })
        .response
        .on_hover_text(
            "The sub-object this collider builds from: one chosen here is no longer drawn",
        );
    if let Some(other) = mesh {
        ui.weak(format!("{other} is not drawn"));
    }
}

/// Import the source file and instantiate it as Unity's model prefab (#1083): a
/// root named after the file and one child per node, each mesh node carrying a
/// path-based `"Asset"` mesh reference (`path::sub_object`) — a `_LOD<n>` set as one
/// LODGroup entity (#472). Routes through the shared `authoring::instantiate_model`,
/// the verb `Scene.Instantiate` of a bare model path drives too, so editor and
/// API never drift. The
/// geometry is re-imported on every scene load; no GPU buffers are persisted.
fn instantiate(editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let asset = match rusty_asset::import_and_sync_sidecar(Path::new(path)) {
        Ok(a) => a,
        Err(_) => return,
    };
    let root = rusty_scene::authoring::instantiate_model(scene, path, &asset, glam::Vec3::ZERO);
    editor.is_dirty = true;
    editor.selected_entity_id = Some(root);
    editor.selected_asset_path = None;
}

#[cfg(test)]
#[path = "model_material_tests.rs"]
mod model_material_tests;
