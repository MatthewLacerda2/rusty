//! The project picker `editor-capture --picker` draws (#854): a sample recent list
//! over folders made under the temp dir, dated against a fixed "now", so the capture
//! is the same on every run and never reads the user's own list.

use std::path::{Path, PathBuf};

use rusty::core::project::{EngineCheck, ProjectFile, ENGINE_COMMIT, PROJECT_FILE};
use rusty::editor::project_picker::recent::RecentProjects;
use rusty::editor::project_picker::ProjectPicker;

/// The fixed "now" the sample is dated against (Unix seconds).
pub const NOW: u64 = 1_800_000_000;
/// The pages besides the list: `--picker <page>`.
pub const PAGES: &[&str] = &["new", "open", "mismatch"];

const MINUTE: u64 = 60;
const DAY: u64 = 86_400;
/// An engine commit that is not this one, for the mismatch prompt.
const OTHER_ENGINE: &str = "0123456789abcdef0123456789abcdef01234567";

/// The sample picker on `page` (`projects`, or one of [`PAGES`]).
pub fn build(page: &str) -> Result<ProjectPicker, String> {
    let root = std::env::temp_dir().join("rusty-picker-capture");
    let games = root.join("games");
    let mut recent = RecentProjects::default();
    // Oldest first: each touch goes to the top.
    recent.touch(&root.join("old").join("prototype"), NOW - 62 * DAY);
    recent.touch(&project(&games, "sandbox", ENGINE_COMMIT)?, NOW - 9 * DAY);
    recent.touch(
        &project(&games, "offline-cs", ENGINE_COMMIT)?,
        NOW - 3 * DAY,
    );
    let horde = project(&games, "horde", OTHER_ENGINE)?;
    recent.touch(&horde, NOW - 5 * MINUTE);
    let mut picker = ProjectPicker::new(recent, None, root);
    match page {
        "new" => picker.show_new(),
        "open" => picker.show_open(),
        "mismatch" => {
            let check = EngineCheck {
                recorded: Some(OTHER_ENGINE.to_string()),
                running: ENGINE_COMMIT.to_string(),
            };
            let warning = check
                .warning()
                .unwrap_or_else(|| "another engine".to_string());
            picker.confirm = Some((horde, warning));
        }
        _ => {}
    }
    Ok(picker)
}

/// A project folder `name` under `parent`, last opened by `commit`.
fn project(parent: &Path, name: &str, commit: &str) -> Result<PathBuf, String> {
    let dir = parent.join(name);
    std::fs::create_dir_all(dir.join("assets")).map_err(|e| e.to_string())?;
    let file = ProjectFile {
        engine_commit: commit.to_string(),
        ..ProjectFile::default()
    };
    file.write(&dir.join(PROJECT_FILE))?;
    Ok(dir)
}
