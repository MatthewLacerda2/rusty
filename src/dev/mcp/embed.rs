//! src/dev/mcp/embed.rs — the embed-mode backend: a headless session that can move
//! to another project (#942).
//!
//! Unity's model is one open project per editor process, and so is rusty's: opening
//! another project closes this one (its unsaved scene edits are lost, as Unity's
//! *Open Project* loses them) and boots a fresh session on the new root, exactly as
//! `session-mcp --project <root>` would have.

use std::cell::RefCell;
use std::path::Path;

use super::EvalBackend;
use crate::core::project::{self, Access};
use crate::dev::session::Session;

/// The in-process session `session-mcp` drives without `--attach`.
pub struct Embedded {
    session: RefCell<Session>,
}

impl Embedded {
    pub fn new(session: Session) -> Self {
        Self {
            session: RefCell::new(session),
        }
    }
}

impl EvalBackend for Embedded {
    fn eval(&self, line: &str) -> Result<String, String> {
        self.session.borrow_mut().command(line)
    }

    fn reopen(&self, root: &Path) -> Option<Result<(), String>> {
        Some(self.switch(root))
    }

    fn take_warnings(&self) -> Vec<String> {
        self.session.borrow().take_warnings()
    }
}

impl Embedded {
    /// Open `root` (it becomes the working directory) and boot its seeded default
    /// scene. On failure the old project stays open and current.
    fn switch(&self, root: &Path) -> Result<(), String> {
        let back = std::env::current_dir().map_err(|e| e.to_string())?;
        project::open(root, Access::Edit)?;
        let boot_scene = crate::scene::seed_default_scene();
        match Session::new(&boot_scene) {
            Ok(session) => {
                *self.session.borrow_mut() = session;
                Ok(())
            }
            Err(err) => {
                project::return_to(&back).ok();
                Err(err)
            }
        }
    }
}
