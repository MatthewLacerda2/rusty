#[cfg(feature = "dev")]
pub mod bake;
#[cfg(not(feature = "dev"))]
mod bake {
    /// No-op bake buttons in a ship build (the bakes are dev-only actions).
    pub fn draw(
        _: &mut egui::Ui,
        _: &mut crate::editor::EditorUi,
        _: &mut crate::scene::Scene,
        _: &mut crate::scripting::ConsoleLogs,
        _: &crate::navigation::NavigationGraph,
        _: &str,
    ) {
    }
}

use crate::editor::EditorUi;
use crate::navigation::{NavBounds, NavigationGraph};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use egui_phosphor::regular as icon;
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

    ui.heading(format!("{}  Scene: {}", icon::FILM_SLATE, filename));
    ui.add_space(5.0);

    draw_metadata(ui, path);
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_scene_operations(ui, editor, scene, console, path, filename);
    draw_navmesh(ui, scene, nav);
    bake::draw(ui, editor, scene, console, nav, path);
}

/// The per-scene Navmesh bake settings section (#276): edit agent radius, agent height,
/// max slope, and max step (Unity's per-scene navmesh bake parameters), then re-bake the
/// shared nav graph from the live scene so the edit takes effect — the SAME `nav.bake(scene)`
/// path the entity inspector's collider/static toggles trigger via `pending_nav_bake`,
/// no second re-bake mechanism. `agent_radius` is live (#277): the re-bake erodes the
/// walkable surface inward by the radius (clearance off walls, thin passages close).
/// `agent_height` is live (#278): the re-bake drops spans whose open space is below the
/// height (no pathing under a low overhang / through a crawlspace).
fn draw_navmesh(ui: &mut egui::Ui, scene: &mut Scene, nav: &mut NavigationGraph) {
    ui.add_space(8.0);
    egui::CollapsingHeader::new(format!("{}  Navmesh", icon::PATH))
        .default_open(false)
        .show(ui, |ui| {
            // Edit the settings behind a scoped borrow so it has dropped before the
            // re-bake (which needs `&Scene`) below.
            let baked = nav.bounds();
            let changed = {
                let s = &mut scene.nav_settings;
                let mut changed = false;
                changed |= ui
                    .add(egui::Slider::new(&mut s.agent_radius, 0.0..=5.0).text("Agent Radius"))
                    .on_hover_text("Erodes the walkable surface inward by this radius on bake")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.agent_height, 0.0..=5.0).text("Agent Height"))
                    .on_hover_text("Drops floors with less open space above them than this height")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.max_slope, 0.0..=4.0).text("Max Slope"))
                    .on_hover_text("Max walkable grade (rise per unit travelled)")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.max_step, 0.0..=4.0).text("Max Step"))
                    .on_hover_text("Max step height between adjacent floors (stairs, curbs)")
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut s.grid_spacing, 0.25..=4.0).text("Grid Spacing"))
                    .on_hover_text("Cell size; smaller = finer grid (re-shapes the grid)")
                    .changed();
                changed |= draw_link_generation(ui, s);
                changed | draw_nav_bounds(ui, &mut s.bounds, baked)
            };
            // Re-bake on apply through the shared bake path, so the knobs take effect
            // immediately exactly like an entity's collider/static edit does.
            if changed {
                nav.bake(scene);
            }
            super::nav_areas::draw(ui, scene, nav);
        });
}

/// The off-mesh link generation knobs (#462): drop height, jump distance and jump
/// height (`0` turns each off), and the spacing between links along an edge — the
/// same fields `Navigation.SetDropHeight` & co. write. Returns whether any changed.
fn draw_link_generation(ui: &mut egui::Ui, s: &mut crate::navigation::NavMeshSettings) -> bool {
    let rows = [
        (
            &mut s.drop_height,
            0.0..=20.0,
            "Drop Height",
            "Generate drops off ledges up to this height (0: off)",
        ),
        (
            &mut s.jump_distance,
            0.0..=10.0,
            "Jump Distance",
            "Generate jumps across gaps up to this wide (0: off)",
        ),
        (
            &mut s.jump_height,
            0.0..=5.0,
            "Jump Height",
            "Generate jumps up ledges up to this high (0: off)",
        ),
        (
            &mut s.link_spacing,
            0.25..=10.0,
            "Link Spacing",
            "One generated link per this many metres of edge",
        ),
    ];
    let mut changed = false;
    for (value, range, label, hint) in rows {
        changed |= ui
            .add(egui::Slider::new(value, range).text(label))
            .on_hover_text(hint)
            .changed();
    }
    changed
}

/// The navmesh bounds (#452): the baked area read-out, plus an "Override Bounds" toggle
/// that authors `nav_settings.bounds` (seeded from the baked area) — the same field
/// `Navigation.SetBounds`/`ClearBounds` write. Returns whether anything changed.
fn draw_nav_bounds(ui: &mut egui::Ui, bounds: &mut Option<NavBounds>, baked: NavBounds) -> bool {
    ui.label(format!(
        "Baked bounds: x [{}, {}]  z [{}, {}]",
        baked.min_x, baked.max_x, baked.min_z, baked.max_z
    ));
    let mut authored = bounds.is_some();
    let mut changed = ui
        .checkbox(&mut authored, "Override Bounds")
        .on_hover_text("Author the navmesh area instead of deriving it from static geometry")
        .changed();
    if changed {
        *bounds = authored.then_some(baked);
    }
    if let Some(b) = bounds {
        for (label, min, max) in [
            ("X", &mut b.min_x, &mut b.max_x),
            ("Z", &mut b.min_z, &mut b.max_z),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                changed |= ui.add(egui::DragValue::new(min).prefix("min ")).changed();
                changed |= ui.add(egui::DragValue::new(max).prefix("max ")).changed();
            });
        }
    }
    changed
}

/// File metadata card: path, on-disk size, plus the entity count and skybox
/// peeked out of the scene's JSON without fully loading it.
fn draw_metadata(ui: &mut egui::Ui, path: &str) {
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

    super::metadata::draw_metadata_card(ui, path, |ui| {
        ui.label(format!("Entities serialized: {}", entity_count));
        ui.label(format!("Skybox: {}", skybox_str));
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
        .add(
            egui::Button::new(format!("{}  Load Scene", icon::FOLDER_OPEN))
                .min_size(egui::Vec2::new(120.0, 24.0)),
        )
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
        .add(
            egui::Button::new(format!("{}  Overwrite with Current", icon::FLOPPY_DISK))
                .min_size(egui::Vec2::new(120.0, 24.0)),
        )
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
