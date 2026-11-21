use std::cell::RefCell;
use std::path::Path;

use super::EvalBackend;
use crate::session::{self, Session};
use rusty_app::ProjectSource;
use rusty_core::project::{self, Access};

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
    fn switch(&self, root: &Path) -> Result<(), String> {
        let back = std::env::current_dir().map_err(|e| e.to_string())?;
        let opened = project::open(root, Access::Edit)?;
        let boot_scene = session::seeded_boot_scene(&opened);
        match Session::boot(&boot_scene, ProjectSource::bound(&opened)) {
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
