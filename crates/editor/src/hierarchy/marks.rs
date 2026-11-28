//! Which loaded scene files hold unsaved edits, for the hierarchy's group headers. Comparing a
//! file with its live document serialises the scene, so it runs once an edit's frames end (a
//! drag edits every frame), and when the loaded set changes.

use std::collections::BTreeSet;

use rusty_scene::Scene;

/// Frames without an edit before the files are compared again.
const SETTLE: u8 = 3;

#[derive(Default)]
pub struct SceneMarks {
    unsaved: BTreeSet<String>,
    paths: Vec<String>,
    wait: Option<u8>,
}

impl SceneMarks {
    pub fn edited(&mut self) {
        self.wait = Some(SETTLE);
    }

    pub fn saved(&mut self) {
        self.wait = Some(0);
    }

    pub fn is_unsaved(&self, path: &str) -> bool {
        self.unsaved.contains(path)
    }

    pub fn refresh(&mut self, scene: &Scene) {
        let paths: Vec<String> = scene.loaded.paths(None).into_iter().flatten().collect();
        if paths != self.paths {
            self.paths = paths;
            self.wait = Some(0);
        }
        match self.wait {
            Some(0) if !scene.is_playing() => {
                let unsaved = self
                    .paths
                    .iter()
                    .filter(|p| rusty_edit::has_unsaved_edits(scene, p));
                self.unsaved = unsaved.cloned().collect();
                self.wait = None;
            }
            Some(n) if n > 0 => self.wait = Some(n - 1),
            _ => {}
        }
    }
}
