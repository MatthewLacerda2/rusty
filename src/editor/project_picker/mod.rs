//! src/editor/project_picker/ — the editor's project picker on start (#854).
//!
//! Unity Hub's model: the editor started without `--project` shows the projects
//! opened before, *New Project* and *Open*, and opens nothing until one is chosen.
//! This is the egui half (what is drawn and what a click means); the shell's
//! picker stage (`shell::editor::picker`) runs it on the window before any project
//! is open, opens the chosen folder, and hands the window to the editor.
//!
//! [`ProjectPicker::draw`] returns the folder to open only once it is safe to: an
//! existing folder must already be a project ([`core::project::is_project`]), and
//! one last opened by another engine asks first ([`check_existing`]). Both rules,
//! and the recent list, are rusty bench's (`core::project::bench`, #942), shared with
//! the session MCP.
//!
//! [`core::project::is_project`]: crate::core::project::is_project

pub mod browse;
mod pages;

use std::path::{Path, PathBuf};

use browse::FolderBrowser;

use crate::core::project::bench::{check_existing, new_project_dir};
use crate::core::project::bench::{recent, RecentProjects};

/// The name *New Project* suggests, Unity Hub's default.
pub const DEFAULT_NEW_NAME: &str = "My project";

/// Which page the picker shows.
pub enum Page {
    /// The recent-projects list.
    Projects,
    /// *New Project*: a parent folder and a name.
    New {
        browser: FolderBrowser,
        name: String,
    },
    /// *Open*: an existing project folder.
    Open(FolderBrowser),
}

/// What a click asked for, applied after the frame is drawn.
pub(crate) enum Action {
    OpenRecent(PathBuf),
    Remove(PathBuf),
    ShowNew,
    ShowOpen,
    Back,
    Create,
    OpenBrowsed,
    ConfirmOpen,
    CancelConfirm,
}

/// The picker's state across frames.
pub struct ProjectPicker {
    pub recent: RecentProjects,
    /// Where removals are saved; `None` keeps them in memory (the capture).
    recent_path: Option<PathBuf>,
    pub page: Page,
    /// A project last opened by another engine, waiting on *Open anyway*: its
    /// folder and the warning shown.
    pub confirm: Option<(PathBuf, String)>,
    /// The last failure, shown under the page until the next action.
    pub error: Option<String>,
    home: PathBuf,
    fonts_installed: bool,
}

impl ProjectPicker {
    /// A picker over `recent`, saving removals to `recent_path`, with `home` as the
    /// browsers' Home.
    pub fn new(recent: RecentProjects, recent_path: Option<PathBuf>, home: PathBuf) -> Self {
        Self {
            recent,
            recent_path,
            page: Page::Projects,
            confirm: None,
            error: None,
            home,
            fonts_installed: false,
        }
    }

    /// This user's picker: their recent list and their home folder.
    pub fn for_user() -> Self {
        let path = recent::default_path();
        let recent = path
            .as_deref()
            .map(RecentProjects::load)
            .unwrap_or_default();
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
        Self::new(recent, path, home)
    }

    /// Show the *New Project* page, browsing next to the latest project.
    pub fn show_new(&mut self) {
        let browser = FolderBrowser::new(self.start_dir(), self.home.clone());
        let name = DEFAULT_NEW_NAME.to_string();
        self.page = Page::New { browser, name };
    }

    /// Show the *Open* page, browsing next to the latest project.
    pub fn show_open(&mut self) {
        self.page = Page::Open(FolderBrowser::new(self.start_dir(), self.home.clone()));
    }

    /// Report that opening the chosen folder failed; the picker stays up.
    pub fn fail(&mut self, err: String) {
        self.error = Some(err);
    }

    /// Draw one frame. `now` (Unix seconds) dates the list. Returns the folder to
    /// open once one is chosen and, if needed, confirmed.
    pub fn draw(&mut self, ctx: &egui::Context, now: u64) -> Option<PathBuf> {
        if !self.fonts_installed {
            crate::editor::theme::Theme::install_fonts(ctx);
            self.fonts_installed = true;
        }
        crate::editor::theme::Theme::dark().apply(ctx);
        let action = pages::draw(self, ctx, now)?;
        self.apply(action)
    }

    fn apply(&mut self, action: Action) -> Option<PathBuf> {
        self.error = None;
        match action {
            Action::OpenRecent(dir) => return self.request_existing(dir),
            Action::Remove(dir) => self.remove_recent(&dir),
            Action::ShowNew => self.show_new(),
            Action::ShowOpen => self.show_open(),
            Action::Back => self.page = Page::Projects,
            Action::Create => {
                let Page::New { browser, name } = &self.page else {
                    return None;
                };
                let made = new_project_dir(browser.dir(), name);
                return made.map_err(|e| self.error = Some(e)).ok();
            }
            Action::OpenBrowsed => {
                let Page::Open(browser) = &self.page else {
                    return None;
                };
                return self.request_existing(browser.dir().to_path_buf());
            }
            Action::ConfirmOpen => return self.confirm.take().map(|(dir, _)| dir),
            Action::CancelConfirm => self.confirm = None,
        }
        None
    }

    /// Open an existing folder: refused unless it is a project, held for a yes when
    /// another engine last opened it.
    fn request_existing(&mut self, dir: PathBuf) -> Option<PathBuf> {
        match check_existing(&dir) {
            Ok(None) => Some(dir),
            Ok(Some(warning)) => {
                self.confirm = Some((dir, warning));
                None
            }
            Err(err) => {
                self.error = Some(err);
                None
            }
        }
    }

    fn remove_recent(&mut self, dir: &Path) {
        self.recent.remove(dir);
        if let Some(path) = &self.recent_path {
            if let Err(err) = self.recent.save(path) {
                self.error = Some(format!("Could not update the recent projects: {err}"));
            }
        }
    }

    /// Where the browsers start: the folder holding the latest project that still
    /// exists, else home.
    fn start_dir(&self) -> PathBuf {
        self.recent
            .projects
            .iter()
            .filter(|p| !p.is_missing())
            .find_map(|p| p.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.home.clone())
    }
}

#[cfg(test)]
mod tests;
