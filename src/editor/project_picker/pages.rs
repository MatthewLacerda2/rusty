//! src/editor/project_picker/pages.rs — drawing the picker's pages (#854).
//!
//! Laid out like Unity Hub's Projects page: a title strip with *Open* and *New
//! Project* on the right, then one row per recent project (name, path, when it was
//! last opened). Each page returns the [`Action`] a click asked for; the picker
//! applies it after the frame.

use egui::{Align, Layout, RichText, Ui};
use egui_phosphor::regular as icon;

use super::{list, Action, Page, ProjectPicker};
use crate::editor::theme::{self, chrome, fonts, Theme};

/// The title strip's text size.
const TITLE_SIZE: f32 = 20.0;
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
                Page::Projects => list::projects(ui, &t, picker, now),
                Page::New { browser, name } => {
                    let mut act = None;
                    header(ui, &t, "New Project", |_| {});
                    ui.label("Project name");
                    let field = ui.add(egui::TextEdit::singleline(name).desired_width(360.0));
                    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        act = Some(Action::Create);
                    }
                    ui.add_space(t.space_md);
                    ui.label("Location");
                    browser.ui(ui, BROWSER_HEIGHT);
                    let target = browser.dir().join(name.trim());
                    let hint = format!("Creates {}", target.display());
                    ui.label(RichText::new(hint).color(t.text_secondary));
                    footer(ui, &mut act, "Create Project", Action::Create);
                    act.or_else(|| escape_back(ui))
                }
                Page::Open(browser) => {
                    let mut act = None;
                    header(ui, &t, "Open Project", |_| {});
                    ui.label("Pick a project folder (it holds project.rusty)");
                    browser.ui(ui, BROWSER_HEIGHT);
                    footer(ui, &mut act, "Open", Action::OpenBrowsed);
                    act.or_else(|| enter_opens(ui)).or_else(|| escape_back(ui))
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

/// Escape on the New / Open page is *Cancel*.
fn escape_back(ui: &Ui) -> Option<Action> {
    let pressed = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    pressed.then_some(Action::Back)
}

/// Enter on the Open page, with nothing focused and the path field not having used
/// it, opens the browsed folder.
fn enter_opens(ui: &Ui) -> Option<Action> {
    if ui.memory(|m| m.focused()).is_some() {
        return None;
    }
    let pressed = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    pressed.then_some(Action::OpenBrowsed)
}

/// The title strip: `title` left, `buttons` right, then a hairline.
pub(super) fn header(ui: &mut Ui, t: &Theme, title: &str, buttons: impl FnOnce(&mut Ui)) {
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
        let (enter, escape) = ui.input_mut(|i| {
            let enter = i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            (
                enter,
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if enter {
            act = Some(Action::ConfirmOpen);
        } else if escape {
            act = Some(Action::CancelConfirm);
        }
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
