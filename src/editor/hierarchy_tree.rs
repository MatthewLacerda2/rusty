//! Scene-hierarchy tree rendering (VS Code Explorer style): a genuine collapsible
//! tree with per-entity expand state, type glyphs and full-row selection. Kept in
//! its own module so `hierarchy.rs` stays under the size gate.

use egui::collapsing_header::CollapsingState;
use egui_phosphor::regular as icon;

use crate::editor::theme::Theme;
use crate::scene::{Entity, Scene};

/// Draw `entity_id` and (when expanded) its subtree. `sel_entity` / `sel_asset`
/// are the editor's selection fields; clicking a row updates them.
pub fn draw_node(
    ui: &mut egui::Ui,
    scene: &Scene,
    entity_id: u32,
    sel_entity: &mut Option<u32>,
    sel_asset: &mut Option<String>,
    t: Theme,
) {
    let (name, children, glyph, glyph_color) = match scene.get_entity(entity_id) {
        Some(e) => (
            e.name.clone(),
            e.children.clone(),
            node_glyph(&e),
            node_color(&e, t),
        ),
        None => return,
    };
    let is_selected = *sel_entity == Some(entity_id);

    // The clickable row: type glyph + name. Selecting toggles off to None (which
    // the inspector reads as "show scene settings"). Right-click offers "Save as
    // Prefab", which extracts this entity's subtree to a `.prefab` asset (#215).
    let mut row = |ui: &mut egui::Ui| {
        ui.colored_label(glyph_color, glyph);
        let label = ui.selectable_label(is_selected, &name);
        if label.clicked() {
            if is_selected {
                *sel_entity = None;
            } else {
                *sel_entity = Some(entity_id);
                *sel_asset = None;
            }
        }
        label.context_menu(|ui| {
            if ui
                .button(format!("{}  Save as Prefab", icon::PACKAGE))
                .clicked()
            {
                save_as_prefab(scene, entity_id, &name);
                ui.close_menu();
            }
        });
    };

    if children.is_empty() {
        // Leaf: indent to line up with the parents' twisty, then the row.
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().icon_width);
            row(ui);
        });
    } else {
        // Branch: a real collapsible node. egui's body indent draws the guide line.
        let id = ui.make_persistent_id(("hierarchy_node", entity_id));
        CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, row)
            .body(|ui| {
                for child in children {
                    draw_node(ui, scene, child, sel_entity, sel_asset, t);
                }
            });
    }
}

/// Extract `entity_id`'s subtree to `project/prefabs/{name}.prefab` via the shared
/// `scene::save_prefab` verb (the same path `Scene.SavePrefab` takes). The folder is
/// created on demand; the entity name is sanitised into a filesystem-safe stem.
fn save_as_prefab(scene: &Scene, entity_id: u32, name: &str) {
    let dir = std::path::Path::new(crate::editor::content_browser::ROOT).join("prefabs");
    std::fs::create_dir_all(&dir).ok();
    let stem: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = if stem.is_empty() { "Prefab" } else { &stem };
    let path = dir.join(format!("{stem}.{}", crate::scene::PREFAB_EXTENSION));
    let _ = crate::scene::save_prefab(scene, entity_id, &path.to_string_lossy());
}

/// Monochrome type glyph for an entity, mirroring VS Code's restrained icons.
fn node_glyph(e: &Entity) -> &'static str {
    if e.health.is_some() {
        icon::HEART
    } else if e.camera.is_some() {
        icon::VIDEO_CAMERA
    } else if e.light.is_some() {
        icon::LIGHTBULB
    } else if e.mesh.is_some() {
        icon::CUBE
    } else {
        icon::CIRCLE
    }
}

/// Glyph tint: dead entities read as danger, lights as the yellow accent, the rest
/// as muted secondary text.
fn node_color(e: &Entity, t: Theme) -> egui::Color32 {
    if e.health.as_ref().map(|h| h.is_dead).unwrap_or(false) {
        t.danger
    } else if e.light.is_some() {
        t.accent_yellow
    } else {
        t.text_secondary
    }
}
