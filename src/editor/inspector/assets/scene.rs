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
    draw_navmesh(ui, scene, nav);
    draw_bake_lighting(ui, scene, console, nav, path);
}

/// The per-scene Navmesh bake settings section (#276): edit agent radius, agent height,
/// max slope, and max step (Unity's per-scene navmesh bake parameters), then re-bake the
/// shared nav graph from the live scene so the edit takes effect — the SAME `nav.bake(scene)`
/// path the entity inspector's collider/static toggles trigger via `pending_nav_bake`,
/// no second re-bake mechanism. `agent_radius` is live (#277): the re-bake erodes the
/// walkable surface inward by the radius (clearance off walls, thin passages close).
/// `agent_height` is live (#278): the re-bake carves cells whose overhead clearance is
/// below the height (no pathing under a low overhang / through a crawlspace).
fn draw_navmesh(ui: &mut egui::Ui, scene: &mut Scene, nav: &mut NavigationGraph) {
    ui.add_space(8.0);
    egui::CollapsingHeader::new("🧭 Navmesh")
        .default_open(false)
        .show(ui, |ui| {
            // Edit the settings behind a scoped borrow so it has dropped before the
            // re-bake (which needs `&Scene`) below.
            let changed = {
                let s = &mut scene.nav_settings;
                let mut changed = false;
                changed |= ui
                    .add(egui::Slider::new(&mut s.agent_radius, 0.0..=5.0).text("Agent Radius"))
                    .on_hover_text("Erodes the walkable surface inward by this radius on bake")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.agent_height, 0.0..=5.0).text("Agent Height"))
                    .on_hover_text("Carves cells with less overhead clearance than this height")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.max_slope, 0.0..=4.0).text("Max Slope"))
                    .on_hover_text("Max walkable grade (rise per unit travelled)")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.max_step, 0.0..=4.0).text("Max Step"))
                    .on_hover_text("Max step height between adjacent cells (stairs, curbs)")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.grid_spacing, 0.25..=4.0).text("Grid Spacing"))
                    .on_hover_text("Cell size; smaller = finer grid (re-shapes the grid)")
                    .changed();
                changed
            };
            // Re-bake on apply through the shared bake path, so the knobs take effect
            // immediately exactly like an entity's collider/static edit does.
            if changed {
                nav.bake(scene);
            }
        });
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
