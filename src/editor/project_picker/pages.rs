//! src/editor/project_picker/pages.rs — drawing the picker's pages (#854).
//!
//! Laid out like Unity Hub's Projects page: a title strip with *Open* and *New
//! Project* on the right, then one row per recent project (name, path, when it was
//! last opened). Each page returns the [`Action`] a click asked for; the picker
//! applies it after the frame.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::regular as icon;

use super::recent::{ago, RecentProject};
use super::{Action, Page, ProjectPicker};
use crate::editor::theme::{self, chrome, fonts, Theme};

/// The title strip's text size.
const TITLE_SIZE: f32 = 20.0;
/// A recent project row's height.
const ROW_HEIGHT: f32 = 46.0;
/// The folder list's height on the New and Open pages.
const BROWSER_HEIGHT: f32 = 320.0;

/// Draw the picker; `Some` when a click asked for something.
pub(super) fn draw(picker: &mut ProjectPicker, ctx: &egui::Context, now: u64) -> Option<Action> {
    let t = theme::from_ctx(ctx);
    let mut action = None;
    let mut root = chrome::root_ui(ctx);
    let frame = egui::Frame::NONE
        .fill(t.bg_tier1)
        .inner_margin(egui::vec2(t.space_lg * 2.0, t.space_lg * 1.5));
    egui::CentralPanel::default()
        .frame(frame)
        .show(&mut root, |ui| {
            action = match &mut picker.page {
                Page::Projects => projects(ui, &t, &picker.recent.projects, now),
                Page::New { browser, name } => {
                    let mut act = None;
                    header(ui, &t, "New Project", |_| {});
                    ui.label("Project name");
                    ui.add(egui::TextEdit::singleline(name).desired_width(360.0));
                    ui.add_space(t.space_md);
                    ui.label("Location");
                    browser.ui(ui, BROWSER_HEIGHT);
                    let target = browser.dir().join(name.trim());
                    let hint = format!("Creates {}", target.display());
                    ui.label(RichText::new(hint).color(t.text_secondary));
                    footer(ui, &mut act, "Create Project", Action::Create);
                    act
                }
                Page::Open(browser) => {
                    let mut act = None;
                    header(ui, &t, "Open Project", |_| {});
                    ui.label("Pick a project folder (it holds project.rusty)");
                    browser.ui(ui, BROWSER_HEIGHT);
                    footer(ui, &mut act, "Open", Action::OpenBrowsed);
                    act
                }
            };
            if let Some(err) = &picker.error {
                ui.add_space(t.space_sm);
                let line = format!("{}  {err}", icon::WARNING_CIRCLE);
                ui.label(RichText::new(line).color(t.danger));
            }
        });
    if let Some((dir, warning)) = &picker.confirm {
        action = confirm(ctx, &t, dir, warning).or(action);
    }
    action
}

/// The title strip: `title` left, `buttons` right, then a hairline.
fn header(ui: &mut Ui, t: &Theme, title: &str, buttons: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let font = fonts::semibold(ui.ctx(), TITLE_SIZE);
        ui.label(RichText::new(title).font(font));
        ui.with_layout(Layout::right_to_left(Align::Center), buttons);
    });
    ui.add_space(t.space_sm);
    chrome::hairline(ui, t);
    ui.add_space(t.space_sm);
}

/// The New / Open page's buttons: Cancel back to the list, and `verb`.
fn footer(ui: &mut Ui, act: &mut Option<Action>, verb: &str, on_verb: Action) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button(RichText::new(verb).strong()).clicked() {
            *act = Some(on_verb);
        }
        if ui.button("Cancel").clicked() {
            *act = Some(Action::Back);
        }
    });
}

/// The Projects page: the title strip and the recent list.
fn projects(ui: &mut Ui, t: &Theme, list: &[RecentProject], now: u64) -> Option<Action> {
    let mut act = None;
    header(ui, t, "Projects", |ui| {
        if ui.button(format!("{}  New Project", icon::PLUS)).clicked() {
            act = Some(Action::ShowNew);
        }
        if ui.button(format!("{}  Open", icon::FOLDER_OPEN)).clicked() {
            act = Some(Action::ShowOpen);
        }
    });
    if list.is_empty() {
        let hint = "No projects yet. Make one with New Project, or Open a project folder.";
        ui.label(RichText::new(hint).color(t.text_secondary));
        return act;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for project in list {
                if let Some(a) = row(ui, t, project, now) {
                    act = Some(a);
                }
            }
        });
    act
}

/// One recent project: name and path, when it was opened, and Remove. A missing
/// folder is greyed out and can only be removed.
fn row(ui: &mut Ui, t: &Theme, project: &RecentProject, now: u64) -> Option<Action> {
    let missing = project.is_missing();
    let (name_color, path_color) = if missing {
        (t.text_secondary, t.outline)
    } else {
        (t.text_primary, t.text_secondary)
    };
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::click());
    if response.hovered() && !missing {
        ui.painter().rect_filled(rect, 4.0, t.bg_hover);
    }
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
                if chrome::icon_button(ui, icon::X, "Remove from list").clicked() {
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
    act
}

/// The engine-mismatch prompt (#853), Unity Hub's "different editor version" one:
/// opening records this engine in the project, so it asks first.
fn confirm(ctx: &egui::Context, t: &Theme, dir: &std::path::Path, warning: &str) -> Option<Action> {
    let mut act = None;
    egui::Modal::new(egui::Id::new("rusty.picker.engine_mismatch")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        let title = format!("{}  Open with a different engine?", icon::WARNING);
        ui.label(
            RichText::new(title)
                .font(fonts::semibold(ui.ctx(), 15.0))
                .color(t.warning),
        );
        ui.add_space(t.space_sm);
        ui.label(format!("{}: {warning}.", dir.display()));
        ui.label("Opening it records this engine in its project.rusty.");
        ui.add_space(t.space_md);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Open anyway").strong()).clicked() {
                act = Some(Action::ConfirmOpen);
            }
            if ui.button("Cancel").clicked() {
                act = Some(Action::CancelConfirm);
            }
        });
    });
    act
}
