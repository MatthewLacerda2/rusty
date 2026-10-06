//! src/core/project/file.rs — `project.rusty`, the file that marks a project (#853).
//!
//! One JSON file at the project root, Unreal's `.uproject` and Unity's
//! `ProjectVersion.txt` plus Player Settings in one:
//!
//! ```json
//! { "engine_commit": "<git commit>", "build": { "startup_scene": "…", … } }
//! ```
//!
//! * **`engine_commit`** — the engine build the project was last opened with, so a
//!   game that lives in its own repo knows which engine it was built against.
//!   [`open`](super::open) compares it with the running build ([`EngineCheck`]) and,
//!   when it may write, records the running one (Unity rewrites its version file the
//!   same way).
//! * **`build`** — the [`BuildSettings`], which lived in `build_settings.json` before
//!   this file. Opening a project that still has that file folds it in and deletes it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::application::BuildSettings;

/// The project file, at the project root.
pub const PROJECT_FILE: &str = "project.rusty";
/// Where the build settings lived before [`PROJECT_FILE`] absorbed them.
pub const LEGACY_BUILD_SETTINGS: &str = "build_settings.json";
/// The engine commit this binary was built from (`build.rs`): a full git hash,
/// `-dirty` when the tree had changes, or [`UNKNOWN_COMMIT`] when built without git.
pub const ENGINE_COMMIT: &str = env!("RUSTY_ENGINE_COMMIT");
/// The commit of a build made without git.
pub const UNKNOWN_COMMIT: &str = "unknown";
const DIRTY: &str = "-dirty";

/// What `project.rusty` holds. Every field is `#[serde(default)]`, so a partial file
/// keeps the defaults for what it omits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    /// The engine commit the project was last opened with.
    pub engine_commit: String,
    /// The project's build settings.
    pub build: BuildSettings,
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            engine_commit: ENGINE_COMMIT.to_string(),
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

    /// What the project at `root` holds, without writing anything: its file, else a
    /// pre-#853 project's `build_settings.json` folded in, else the defaults.
    pub fn load(root: &Path) -> Result<Self, String> {
        if let Some(file) = Self::read(&root.join(PROJECT_FILE))? {
            return Ok(file);
        }
        let legacy = root.join(LEGACY_BUILD_SETTINGS);
        if !legacy.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&legacy)
            .map_err(|e| format!("Failed to read {}: {e}", legacy.display()))?;
        let build = BuildSettings::from_json(&text)?;
        // A pre-#853 project recorded no engine: say so, rather than claim this one.
        let engine_commit = String::new();
        Ok(Self {
            engine_commit,
            build,
        })
    }

    /// Write the file to `path`, unless it already holds exactly this — a project
    /// opened by the same engine is left untouched (no git noise, no write to a
    /// read-only bundle).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = self.to_json();
        if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
            return Ok(());
        }
        std::fs::write(path, text).map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }
}

/// The open-time engine check: which engine the project was last opened with, and
/// which one is running. A frontend decides how to tell the user ([`warning`]);
/// [`open`](super::open) only reports it.
///
/// [`warning`]: EngineCheck::warning
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineCheck {
    /// The commit the project recorded; `None` when it recorded none (a new project,
    /// or one from before `project.rusty`).
    pub recorded: Option<String>,
    /// The running engine's commit ([`ENGINE_COMMIT`]).
    pub running: String,
}

impl EngineCheck {
    /// The check of a project that recorded `recorded` against this build.
    pub fn against(recorded: Option<&str>) -> Self {
        Self {
            recorded: recorded.filter(|c| !c.is_empty()).map(str::to_string),
            running: ENGINE_COMMIT.to_string(),
        }
    }

    /// Whether the project was last opened with this engine, as far as can be told.
    /// Only two known, different commits are a mismatch: no record and an `unknown`
    /// build (on either side) can't be compared, and a `-dirty` suffix is ignored —
    /// local edits on the same commit are the engine developer's normal state, not
    /// version skew.
    pub fn matches(&self) -> bool {
        let Some(recorded) = self.recorded.as_deref() else {
            return true;
        };
        let (recorded, running) = (base(recorded), base(&self.running));
        recorded == running || recorded == UNKNOWN_COMMIT || running == UNKNOWN_COMMIT
    }

    /// The one line to show when the commits differ; `None` when they match.
    pub fn warning(&self) -> Option<String> {
        let recorded = self.recorded.as_deref().filter(|_| !self.matches())?;
        Some(format!(
            "this project was last opened with engine {}, this is engine {}: \
             scenes and scripts may not behave as they did",
            short(recorded),
            short(&self.running)
        ))
    }
}

fn base(commit: &str) -> &str {
    commit.strip_suffix(DIRTY).unwrap_or(commit)
}

/// A commit as people read it: 12 hex digits, plus `-dirty` when it had one.
fn short(commit: &str) -> String {
    let hash = base(commit);
    let dirty = if hash.len() < commit.len() { DIRTY } else { "" };
    format!("{}{dirty}", &hash[..hash.len().min(12)])
}

/// Bring the project file at `root` up to date when the project is opened for
/// editing: create it if missing (folding a legacy `build_settings.json` in and
/// deleting it), and record the running engine. Returns the check made against what
/// the file held before, and one line per change.
pub(super) fn record(root: &Path) -> Result<(EngineCheck, Vec<String>), String> {
    let path = root.join(PROJECT_FILE);
    let existed = path.exists();
    let mut file = ProjectFile::load(root)?;
    let check = EngineCheck::against(Some(&file.engine_commit));
    let mut lines = Vec::new();
    // An `unknown` build can't say which engine it is: keep the last known record.
    if ENGINE_COMMIT != UNKNOWN_COMMIT || file.engine_commit.is_empty() {
        file.engine_commit = ENGINE_COMMIT.to_string();
    }
    file.write(&path)?;
    if !existed {
        lines.push(format!("created {PROJECT_FILE}"));
        let legacy = root.join(LEGACY_BUILD_SETTINGS);
        if legacy.exists() {
            std::fs::remove_file(&legacy)
                .map_err(|e| format!("Failed to remove {}: {e}", legacy.display()))?;
            lines.push(format!(
                "folded {LEGACY_BUILD_SETTINGS} into {PROJECT_FILE}"
            ));
        }
    }
    Ok((check, lines))
}

/// The check alone, for a project opened only to run: nothing is written.
pub(super) fn inspect(root: &Path) -> Result<EngineCheck, String> {
    let file = ProjectFile::load(root)?;
    Ok(EngineCheck::against(Some(&file.engine_commit)))
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod file_tests;
