//! src/core/project/file.rs — `project.rusty`, the file that marks a project (#853).
//!
//! One JSON file at the project root, Unreal's `.uproject` and Unity's
//! `ProjectVersion.txt` plus Player Settings in one:
//!
//! ```json
//! { "engine_commit": "<git commit>", "engine_order": 1318, "build": { … } }
//! ```
//!
//! * **`engine_commit`** — the engine build the project was last opened with, so a
//!   game that lives in its own repo knows which engine it was built against.
//!   [`open`](super::open) compares it with the running build ([`EngineCheck`]) and,
//!   when it may write, records the running one (Unity rewrites its version file the
//!   same way).
//! * **`engine_order`** — that commit's place in history, which tells a newer engine
//!   from an older one (#1319); absent when the engine couldn't count (a shallow
//!   clone).
//! * **`build`** — the [`BuildSettings`].

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::engine::commit::{engine_commit, engine_order, UNKNOWN_COMMIT};
use super::engine::{base, EngineCheck};
use crate::core::application::BuildSettings;

/// The project file, at the project root.
pub const PROJECT_FILE: &str = "project.rusty";

/// What `project.rusty` holds. Every field is `#[serde(default)]`, so a partial file
/// keeps the defaults for what it omits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    /// The engine commit the project was last opened with.
    pub engine_commit: String,
    /// Where that commit sits in history ([`engine_order`]). A file that omits it
    /// reads as `None`, never as the running engine's, so a record from before
    /// counts were kept is an upgrade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_order: Option<u64>,
    /// The project's build settings.
    pub build: BuildSettings,
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            engine_commit: engine_commit().to_string(),
            engine_order: engine_order(),
            build: BuildSettings::default(),
        }
    }
}

impl ProjectFile {
    /// Parse the file's JSON text. Malformed JSON is an error.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("Failed to parse {PROJECT_FILE}: {e}"))
    }

    /// The pretty JSON the file holds (with a trailing newline).
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Read the file at `path`; `None` when there is none.
    pub fn read(path: &Path) -> Result<Option<Self>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        Self::from_json(&text).map(Some)
    }

    /// What the project at `root` holds, without writing anything: its file, else
    /// the defaults.
    pub fn load(root: &Path) -> Result<Self, String> {
        Ok(Self::read(&root.join(PROJECT_FILE))?.unwrap_or_default())
    }

    /// Write the file to `path`, unless it already holds exactly this — a project
    /// opened by the same engine is left untouched (no git noise, no write to a
    /// read-only bundle).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = self.to_json();
        if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
            return Ok(());
        }
        // lint: allow(asset-write) project settings, not read by a path-keyed cache
        std::fs::write(path, text).map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }
}

/// Bring the project file at `root` up to date when the project is opened for
/// editing: create it if missing, and record the running engine. Returns the check
/// made against what the file held before (an upgrade reports itself through it:
/// [`EngineCheck::upgrade`]), the file as it now stands, and one line per change. A file it creates names the product
/// after the project's folder, Unity's default Product Name (#1317); an existing file
/// keeps whatever it says.
pub(super) fn record(root: &Path) -> Result<(EngineCheck, ProjectFile, Vec<String>), String> {
    let path = root.join(PROJECT_FILE);
    let existed = path.exists();
    let mut file = ProjectFile::load(root)?;
    let check = EngineCheck::against(Some(&file.engine_commit), file.engine_order);
    let mut lines = Vec::new();
    if let Some(name) = root.file_name().filter(|_| !existed) {
        file.build.product_name = name.to_string_lossy().into_owned();
    }
    // An `unknown` build can't say which engine it is: keep the last known record.
    // A shallow build can't count: keep the record's count while the commit holds.
    if engine_commit() != UNKNOWN_COMMIT || file.engine_commit.is_empty() {
        if engine_order().is_some() || base(&file.engine_commit) != base(engine_commit()) {
            file.engine_order = engine_order();
        }
        file.engine_commit = engine_commit().to_string();
    }
    file.write(&path)?;
    if !existed {
        lines.push(format!("created {PROJECT_FILE}"));
    }
    Ok((check, file, lines))
}

/// The check and the file, for a project opened only to run: nothing is written.
pub(super) fn inspect(root: &Path) -> Result<(EngineCheck, ProjectFile), String> {
    let file = ProjectFile::load(root)?;
    let check = EngineCheck::against(Some(&file.engine_commit), file.engine_order);
    Ok((check, file))
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod file_tests;
