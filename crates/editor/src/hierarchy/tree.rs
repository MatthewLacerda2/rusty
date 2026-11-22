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

pub(super) struct Rows<'a> {
    pub sel_entity: &'a mut Option<u32>,
    pub sel_asset: &'a mut Option<String>,
    pub delete: &'a mut Option<u32>,
    pub theme: Theme,
}

impl Rows<'_> {
    pub(super) fn draw(&mut self, ui: &mut egui::Ui, scene: &Scene, flat: &[Row]) {
        let row_h = ui.spacing().interact_size.y;
        let scroll = egui::ScrollArea::vertical()
            .id_salt(SCROLL_SALT)
            .auto_shrink([false, true]);
        scroll.show_rows(ui, row_h, flat.len(), |ui, range| {
            let pitch = row_h + ui.spacing().item_spacing.y;
            let indent = ui.spacing().indent;
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

    fn draw_row(&mut self, ui: &mut egui::Ui, scene: &Scene, row: &Row, rect: egui::Rect) {
        let Some(name) = scene.world.name(row.id) else {
            return;
        };
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        let builder = egui::UiBuilder::new()
            .id_salt(("hierarchy_row", row.id))
            .max_rect(rect)
            .layout(layout);
        let ui = &mut ui.new_child(builder);
        if row.open.is_some() {
            twisty(ui, scene, row.id);
        } else {
            ui.add_space(ui.spacing().icon_width);
        }
        self.row(ui, scene, row.id, &name);
    }

    fn row(&mut self, ui: &mut egui::Ui, scene: &Scene, entity_id: u32, name: &str) {
        let (glyph, glyph_color) = node_glyph(scene, entity_id, self.theme);
        let is_selected = *self.sel_entity == Some(entity_id);
        ui.colored_label(glyph_color, glyph);
        let label = ui.add(egui::Button::selectable(is_selected, name).truncate());
        let label = full_name_on_hover(ui, label, name);
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

/// A truncated name's tooltip: the full name, measured only while hovered, so
/// a row that is not under the pointer costs nothing for it.
fn full_name_on_hover(ui: &egui::Ui, label: egui::Response, name: &str) -> egui::Response {
    if !label.hovered() {
        return label;
    }
    let font = egui::TextStyle::Button.resolve(ui.style());
    let color = egui::Color32::PLACEHOLDER;
    let width = ui.fonts_mut(|f| f.layout_no_wrap(name.to_owned(), font, color).size().x);
    let room = label.rect.width() - 2.0 * ui.spacing().button_padding.x;
    if width > room + 0.5 {
        label.on_hover_text(name)
    } else {
        label
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

fn twisty(ui: &mut egui::Ui, scene: &Scene, entity_id: u32) {
    let mut state = node_state(ui.ctx(), scene, entity_id);
    let spacing = ui.spacing().item_spacing;
    ui.spacing_mut().item_spacing.x = 0.0;
    if state.show_toggle_button(ui, paint_default_icon).clicked() {
        state.store(ui.ctx());
    }
    ui.spacing_mut().item_spacing = spacing;
}

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
