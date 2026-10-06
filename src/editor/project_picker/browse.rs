//! src/editor/project_picker/browse.rs — an in-editor folder browser (#854).
//!
//! The editor has no native file dialogs (no dialog crate; every path it takes is
//! typed or picked in egui, like the content browser), so the picker's *New* and
//! *Open* pages browse folders here: a path field, Up and Home, and the subfolders
//! of the current one. Hidden folders (a leading `.`) are left out, as Finder and
//! most file managers do.

use std::path::{Path, PathBuf};

use egui::{RichText, Ui};
use egui_phosphor::regular as icon;

use crate::editor::theme;

/// The folder being browsed and its subfolders, listed once per move.
pub struct FolderBrowser {
    dir: PathBuf,
    subdirs: Vec<String>,
    /// The path field's text; Enter moves there.
    typed: String,
    home: PathBuf,
}

impl FolderBrowser {
    /// Start at `dir`, with `home` as the Home button's target.
    pub fn new(dir: PathBuf, home: PathBuf) -> Self {
        let mut browser = Self {
            dir: PathBuf::new(),
            subdirs: Vec::new(),
            typed: String::new(),
            home,
        };
        browser.go(dir);
        browser
    }

    /// The folder being browsed.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The subfolders of [`Self::dir`] the list shows, sorted.
    pub fn subdirs(&self) -> &[String] {
        &self.subdirs
    }

    /// Browse `dir`. A folder that can't be read lists nothing.
    pub fn go(&mut self, dir: PathBuf) {
        self.subdirs = list_subdirs(&dir);
        self.typed = dir.display().to_string();
        self.dir = dir;
    }

    /// The path row, then the subfolder list (`height` tall). A click on a subfolder
    /// moves into it.
    pub fn ui(&mut self, ui: &mut Ui, height: f32) {
        let t = theme::from_ui(ui);
        let mut target = None;
        ui.horizontal(|ui| {
            let up = self.dir.parent().map(Path::to_path_buf);
            let up_button = ui.add_enabled(up.is_some(), egui::Button::new(icon::ARROW_UP));
            if up_button.on_hover_text("Up").clicked() {
                target = up;
            }
            if ui.button(icon::HOUSE).on_hover_text("Home").clicked() {
                target = Some(self.home.clone());
            }
            let field = egui::TextEdit::singleline(&mut self.typed).desired_width(f32::INFINITY);
            let response = ui.add(field);
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                target = Some(PathBuf::from(self.typed.trim()));
            }
        });
        egui::Frame::NONE
            .fill(t.bg_tier0)
            .inner_margin(t.space_xs)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.subdirs.is_empty() {
                            ui.label(RichText::new("No subfolders").color(t.text_secondary));
                        }
                        for name in &self.subdirs {
                            let label = format!("{}  {name}", icon::FOLDER);
                            if ui.selectable_label(false, label).clicked() {
                                target = Some(self.dir.join(name));
                            }
                        }
                    });
            });
        if let Some(dir) = target {
            self.go(dir);
        }
    }
}

/// The visible subfolders of `dir`, sorted case-insensitively.
pub fn list_subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
}
