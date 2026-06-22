use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use std::fs;
use std::path::Path;

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &mut NavigationGraph,
    path: &str,
) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("🎬 Scene: {}", filename));
    ui.add_space(5.0);

    draw_metadata_card(ui, path);
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_scene_operations(ui, editor, scene, console, path, filename);
    draw_bake_lighting(ui, scene, console, nav, path);
}

/// The "Bake Lighting" button (#246): auto-place + bake both probe sets through the
/// SAME orchestration the `Lighting.Bake()` script verb uses (editor↔API parity, no
/// second path). Dev-only — the bake drives a headless GPU and writes authoring
/// artifacts (the SH sidecar + KTX2 cubemaps), so it is absent from ship builds, just
/// like the `Lighting.Bake` binding.
#[cfg(feature = "dev")]
fn draw_bake_lighting(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &NavigationGraph,
    path: &str,
) {
    ui.add_space(8.0);
    if !ui
        .add(egui::Button::new("💡 Bake Lighting").min_size(egui::Vec2::new(120.0, 30.0)))
        .on_hover_text("Auto-place + bake light & reflection probes")
        .clicked()
    {
        return;
    }
    let params = crate::dev::lighting_bake::LightingBakeParams::default();
    match crate::dev::lighting_bake::bake_lighting(scene, Some(path), Some(nav), params) {
        Ok(r) => console.info(format!(
            "Baked lighting: {} light probe(s), {} reflection probe(s)",
            r.light_probes, r.reflection_probes
        )),
        Err(e) => console.error(format!("Bake Lighting failed: {}", e)),
    }
}

/// No-op `Bake Lighting` button in a ship build (the bake is a dev-only action).
#[cfg(not(feature = "dev"))]
fn draw_bake_lighting(
    _ui: &mut egui::Ui,
    _scene: &mut Scene,
    _console: &mut ConsoleLogs,
    _nav: &NavigationGraph,
    _path: &str,
) {
}

/// File metadata card: path, on-disk size, plus the entity count and skybox
/// peeked out of the scene's JSON without fully loading it.
fn draw_metadata_card(ui: &mut egui::Ui, path: &str) {
    let size_str = if let Ok(meta) = fs::metadata(path) {
        format_size(meta.len())
    } else {
        "Unknown size".to_string()
    };

    // Attempt to inspect JSON content
    let mut entity_count = 0;
    let mut skybox_str = "None".to_string();
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(arr) = val.get("entities").and_then(|e| e.as_array()) {
                entity_count = arr.len();
            }
            if let Some(skybox) = val.get("skybox_path").and_then(|s| s.as_str()) {
                skybox_str = skybox.to_string();
            }
        }
    }

    egui::Frame::none()
        .fill(crate::editor::theme::from_ui(ui).bg_tier2)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Path: {}", path));
                ui.label(format!("Size: {}", size_str));
                ui.label(format!("Entities serialized: {}", entity_count));
                ui.label(format!("Skybox: {}", skybox_str));
            });
        });
}

/// Scene operations: load the scene file into the world, or overwrite it with
/// the current world state, logging results to the console.
fn draw_scene_operations(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    path: &str,
    filename: &str,
) {
    ui.heading("Scene Operations");
    ui.add_space(8.0);

    // Button to load scene
    if ui
        .add(egui::Button::new("📂 Load Scene").min_size(egui::Vec2::new(120.0, 30.0)))
        .clicked()
    {
        match scene.load_from_file(path) {
            Ok(_) => {
                editor.current_scene_path = Some(path.to_string());
                editor.selected_entity_id = None;
                editor.selected_asset_path = None;
                editor.is_dirty = true;
                console.info(format!("Successfully loaded scene from {}", filename));
            }
            Err(e) => {
                console.error(format!("Failed to load scene: {}", e));
            }
        }
    }

    ui.add_space(8.0);

    // Button to save/overwrite scene
    if ui
        .add(egui::Button::new("💾 Overwrite with Current").min_size(egui::Vec2::new(120.0, 30.0)))
        .clicked()
    {
        match scene.save_to_file(path) {
            Ok(_) => {
                console.info(format!("Overwrote scene file: {}", filename));
            }
            Err(e) => {
                console.error(format!("Failed to save scene: {}", e));
            }
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
