//! Scene-hierarchy tree rendering (VS Code Explorer style): a collapsible tree
//! with per-entity expand state, type glyphs and full-row selection, drawn from
//! `flat`'s rows so only the ones in view are laid out.

use egui::collapsing_header::paint_default_icon;
use egui_phosphor::regular as icon;

use super::flat::{node_state, Row};
use super::glyph::node_glyph;
use crate::theme::Theme;
use rusty_scene::Scene;

/// The tree's scroll area, named so its offset is found again (and by tests).
pub(super) const SCROLL_SALT: &str = "hierarchy_rows";

/// What a row reads and writes besides the scene: the editor's selection fields
/// (clicking a row updates them), the row a context-menu *Delete* asked for
/// (`hierarchy::delete` applies it after the tree is drawn), and the theme.
pub(super) struct Rows<'a> {
    pub sel_entity: &'a mut Option<u32>,
    pub sel_asset: &'a mut Option<String>,
    pub delete: &'a mut Option<u32>,
    pub theme: Theme,
}

impl Rows<'_> {
    /// Draw the flattened tree in a vertical scroll area, laying out only the
    /// rows in its view: one off screen costs its share of the scroll height and
    /// nothing else. Every row is one height, which `show_rows` needs.
    pub(super) fn draw(&mut self, ui: &mut egui::Ui, scene: &Scene, flat: &[Row]) {
        let row_h = ui.spacing().interact_size.y;
        // As wide as the panel, not as the rows: only the rows in view are laid
        // out, so following their width would move the scroll bar while scrolling.
        let scroll = egui::ScrollArea::vertical()
            .id_salt(SCROLL_SALT)
            .auto_shrink([false, true]);
        scroll.show_rows(ui, row_h, flat.len(), |ui, range| {
            let pitch = row_h + ui.spacing().item_spacing.y;
            let indent = ui.spacing().indent;
            // Where row 0 would sit: `show_rows` starts its ui at `range.start`.
            let origin = ui.max_rect().left_top() - egui::vec2(0.0, range.start as f32 * pitch);
            for (i, row) in flat.iter().enumerate().take(range.end).skip(range.start) {
                let min = origin + egui::vec2(row.depth as f32 * indent, i as f32 * pitch);
                let right = ui.max_rect().right();
                let rect = egui::Rect::from_min_max(min, egui::pos2(right, min.y + row_h));
                self.draw_row(ui, scene, row, rect);
            }
            guide_lines(ui, flat, range, origin, pitch);
        });
    }

    /// One row in `rect`, laid out as `ui.horizontal` lays it out: a branch's
    /// twisty or a leaf's matching space, then the glyph and the name. The name
    /// is borrowed from the World for the row's draw, never copied.
    fn draw_row(&mut self, ui: &mut egui::Ui, scene: &Scene, row: &Row, rect: egui::Rect) {
        let Some(name) = scene.world.name(row.id) else {
            return;
        };
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        let builder = egui::UiBuilder::new()
            .id_salt(("hierarchy_row", row.id))
            .max_rect(rect)
            .layout(layout);
        ui.scope_builder(builder, |ui| {
            if row.open.is_some() {
                twisty(ui, scene, row.id);
            } else {
                ui.add_space(ui.spacing().icon_width);
            }
            self.row(ui, scene, row.id, &name);
        });
    }

    /// The clickable row: type glyph + name. Selecting toggles off to None (which
    /// the inspector reads as "show scene settings"). Right-click offers "Save as
    /// Prefab", which extracts this entity's subtree to a `.prefab` asset (#215),
    /// and "Delete", which deletes it and its subtree (#1915).
    fn row(&mut self, ui: &mut egui::Ui, scene: &Scene, entity_id: u32, name: &str) {
        let (glyph, glyph_color) = node_glyph(scene, entity_id, self.theme);
        let is_selected = *self.sel_entity == Some(entity_id);
        ui.colored_label(glyph_color, glyph);
        let label = ui.selectable_label(is_selected, name);
        if label.clicked() {
            if is_selected {
                *self.sel_entity = None;
            } else {
                *self.sel_entity = Some(entity_id);
                *self.sel_asset = None;
            }
        }
        label.context_menu(|ui| {
            let prefab = format!("{}  Save as Prefab", icon::PACKAGE);
            if ui.button(prefab).clicked() {
                save_as_prefab(scene, entity_id, name);
                ui.close();
            }
            if ui.button(format!("{}  Delete", icon::TRASH)).clicked() {
                *self.delete = Some(entity_id);
                ui.close();
            }
        });
    }
}

/// Extract `entity_id`'s subtree to `assets/prefabs/{name}.prefab` via the shared
/// `edit::save_prefab` verb (the same path `Scene.SavePrefab` takes). The folder is
/// created on demand; the entity name is sanitised into a filesystem-safe stem.
fn save_as_prefab(scene: &Scene, entity_id: u32, name: &str) {
    let dir = std::path::Path::new(rusty_core::project::ASSETS_DIR).join("prefabs");
    std::fs::create_dir_all(&dir).ok();
    let stem: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = if stem.is_empty() { "Prefab" } else { &stem };
    let path = dir.join(format!("{stem}.{}", rusty_edit::PREFAB_EXTENSION));
    let _ = rusty_edit::save_prefab(scene, entity_id, &path.to_string_lossy());
}

/// A branch's expand toggle, as `CollapsingState::show_header` draws it: the full
/// indent wide with no spacing after it. A click flips the stored state, and the
/// next frame's walk shows or skips the subtree.
fn twisty(ui: &mut egui::Ui, scene: &Scene, entity_id: u32) {
    let mut state = node_state(ui.ctx(), scene, entity_id);
    let spacing = ui.spacing().item_spacing;
    ui.spacing_mut().item_spacing.x = 0.0;
    if state.show_toggle_button(ui, paint_default_icon).clicked() {
        state.store(ui.ctx());
    }
    ui.spacing_mut().item_spacing = spacing;
}

/// The guide line down an open branch's body, as egui's indent draws it: half an
/// indent in from the branch's row, from the first child's top to 2 px above the
/// last descendant's bottom. Drawn for each open branch with a row in `range`,
/// and for those above it whose body reaches into it (the first row's ancestors).
fn guide_lines(
    ui: &egui::Ui,
    flat: &[Row],
    range: std::ops::Range<usize>,
    origin: egui::Pos2,
    pitch: f32,
) {
    let row_h = ui.spacing().interact_size.y;
    let indent = ui.spacing().indent;
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let line = |i: usize| {
        let row = &flat[i];
        if row.open != Some(true) || row.end <= i + 1 {
            return;
        }
        let x = origin.x + row.depth as f32 * indent + 0.5 * indent;
        let top = origin.y + (i + 1) as f32 * pitch;
        let bottom = origin.y + (row.end - 1) as f32 * pitch + row_h - 2.0;
        ui.painter()
            .line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], stroke);
    };
    let mut above = flat.get(range.start).and_then(|r| r.parent);
    while let Some(p) = above {
        line(p);
        above = flat[p].parent;
    }
    range.for_each(line);
}
