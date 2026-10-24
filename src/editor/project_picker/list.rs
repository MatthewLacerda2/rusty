//! src/editor/project_picker/list.rs — the Projects page: the recent list (#1320).
//!
//! Keyboard first as well as mouse: each row is one Tab stop, Up / Down move between
//! rows, and Enter opens the focused one. The ✕ is mouse-only (not a Tab stop), and a
//! removal leaves an *Undo* line under the list until the next action.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::regular as icon;

use super::pages::header;
use super::{Action, ProjectPicker};
use crate::core::project::bench::recent::{ago, RecentProject};
use crate::editor::theme::{chrome, fonts, Theme};

/// A recent project row's height.
const ROW_HEIGHT: f32 = 46.0;

/// The Projects page: the title strip and the recent list.
pub(super) fn projects(
    ui: &mut Ui,
    t: &Theme,
    picker: &mut ProjectPicker,
    now: u64,
) -> Option<Action> {
    let mut act = None;
    header(ui, t, "Projects", |ui| {
        if ui.button(format!("{}  New Project", icon::PLUS)).clicked() {
            act = Some(Action::ShowNew);
        }
        if ui.button(format!("{}  Open", icon::FOLDER_OPEN)).clicked() {
            act = Some(Action::ShowOpen);
        }
    });
    if picker.removed.is_some() {
        let name = picker.removed.as_ref().map(|(_, p)| p.name());
        ui.horizontal(|ui| {
            let line = format!("Removed {} from the list.", name.unwrap_or_default());
            ui.label(RichText::new(line).color(t.text_secondary));
            if ui.button("Undo").clicked() {
                act = Some(Action::UndoRemove);
            }
        });
        ui.add_space(t.space_xs);
    }
    let list = &picker.recent.projects;
    if list.is_empty() {
        let hint = "No projects yet. Make one with New Project, or Open a project folder.";
        ui.label(RichText::new(hint).color(t.text_secondary));
        return act;
    }
    let mut rows = Vec::with_capacity(list.len());
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for project in list {
                let (a, response) = row(ui, t, project, now);
                act = a.or(act.take());
                rows.push(response);
            }
        });
    if let Some(at) = picker.focus_row.take() {
        if let Some(r) = rows.get(at) {
            r.request_focus();
        }
    }
    move_focus(ui, &rows);
    act
}

/// Up / Down move keyboard focus to the neighbouring row.
fn move_focus(ui: &Ui, rows: &[egui::Response]) {
    let Some(at) = rows.iter().position(|r| r.has_focus()) else {
        return;
    };
    let (up, down) = ui.input_mut(|i| {
        let up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
        (
            up,
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
        )
    });
    let to = if up {
        at.checked_sub(1)
    } else if down {
        Some(at + 1).filter(|&n| n < rows.len())
    } else {
        None
    };
    if let Some(r) = to.and_then(|n| rows.get(n)) {
        r.request_focus();
        r.scroll_to_me(None);
    }
}

/// One recent project: name and path, when it was opened, and Remove. A missing
/// folder is greyed out and can only be removed.
fn row(
    ui: &mut Ui,
    t: &Theme,
    project: &RecentProject,
    now: u64,
) -> (Option<Action>, egui::Response) {
    let missing = project.is_missing();
    let (name_color, path_color) = if missing {
        (t.text_secondary, t.outline)
    } else {
        (t.text_primary, t.text_secondary)
    };
    let width = ui.available_width();
    let sense = if missing {
        egui::Sense::CLICK
    } else {
        egui::Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), sense);
    if response.hovered() && !missing {
        ui.painter().rect_filled(rect, 4.0, t.bg_hover);
    }
    chrome::focus_ring(ui, t, &response);
    let mut act =
        (response.clicked() && !missing).then(|| Action::OpenRecent(project.path.clone()));
    let inner = rect.shrink2(egui::vec2(t.space_md, t.space_xs));
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.horizontal_centered(|ui| {
            ui.vertical(|ui| {
                let font = fonts::semibold(ui.ctx(), 14.0);
                ui.label(RichText::new(project.name()).font(font).color(name_color));
                let path = project.path.display().to_string();
                ui.label(RichText::new(path).small().color(path_color));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let glyph = RichText::new(icon::X).color(t.text_secondary);
                let remove = egui::Button::new(glyph)
                    .frame(false)
                    .sense(egui::Sense::CLICK);
                if ui.add(remove).on_hover_text("Remove from list").clicked() {
                    act = Some(Action::Remove(project.path.clone()));
                }
                ui.add_space(t.space_md);
                let when = if missing {
                    "Missing".to_string()
                } else {
                    ago(project.opened_at, now)
                };
                ui.label(RichText::new(when).color(path_color));
            });
        });
    });
    chrome::hairline(ui, t);
    (act, response)
}
