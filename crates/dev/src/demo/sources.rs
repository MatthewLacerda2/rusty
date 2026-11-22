use std::path::{Path, PathBuf};

use rusty_app::GameWorld;
use rusty_core::hash::content;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(default)]
    pub scripts: String,
}

pub(super) fn warn_changes(world: &GameWorld, header: &super::format::Header) {
    let running = rusty_core::project::engine_commit();
    let mut console = world.console().borrow_mut();
    if header.engine_commit != running {
        let line = format!(
            "[Demo] recorded on engine {}, replayed on {running}: it may differ",
            header.engine_commit
        );
        console.warn(line);
    }
    if let Some(what) = header.sources.changed(&Sources::of(world)) {
        console.warn(format!(
            "[Demo] the project's {what} changed since it was recorded: it may differ"
        ));
    }
}

fn skipped(name: &str) -> bool {
    use rusty_core::project::{CACHE_DIR, SAVED_DIR};
    name.starts_with('.') || [SAVED_DIR, CACHE_DIR, "target"].contains(&name)
}

fn lua_files(root: &Path) -> Vec<PathBuf> {
    let (mut found, mut dirs) = (Vec::new(), vec![root.to_path_buf()]);
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let (path, name) = (entry.path(), entry.file_name());
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir && !skipped(&name.to_string_lossy()) {
                dirs.push(path);
            } else if !is_dir && rusty_core::paths::has_extension(&path, "lua") {
                found.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            }
        }
    }
    found.sort();
    found
}

fn scripts_digest(root: &Path) -> String {
    let mut all = Vec::new();
    for file in lua_files(root) {
        all.extend_from_slice(file.to_string_lossy().as_bytes());
        all.push(0);
        all.extend(std::fs::read(root.join(&file)).unwrap_or_default());
        all.push(0);
    }
    format!("{:016x}", content(&all))
}

impl Sources {
    pub fn of(world: &GameWorld) -> Self {
        let root = world.script_manager().resolve_script("");
        let root = if root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            root
        };
        let scene_path = world.script_manager().scene_path_cell().borrow().clone();
        let scene = scene_path
            .and_then(|path| std::fs::read(path).ok())
            .map(|bytes| format!("{:016x}", content(&bytes)));
        Self {
            scene,
            scripts: scripts_digest(&root),
        }
    }

    pub fn changed(&self, now: &Self) -> Option<&'static str> {
        match (self.scene != now.scene, self.scripts != now.scripts) {
            (true, true) => Some("its scene file and its scripts"),
            (true, false) => Some("its scene file"),
            (false, true) => Some("its scripts"),
            (false, false) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scripts_digest_follows_every_lua_file_but_not_saved_ones() {
        let root = rusty_test_support::temp::dir();
        let brain = root.join("scripts").join("ai").join("brain.lua");
        std::fs::create_dir_all(brain.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.join("saved")).unwrap();
        std::fs::write(&brain, "x = 1").unwrap();
        let before = scripts_digest(&root);
        std::fs::write(root.join("saved").join("notes.lua"), "ignored").unwrap();
        std::fs::write(root.join("readme.txt"), "ignored").unwrap();
        assert_eq!(scripts_digest(&root), before, "saved/ and non-Lua files");
        std::fs::write(&brain, "x = 2").unwrap();
        assert_ne!(scripts_digest(&root), before, "an edited script");
    }
}
