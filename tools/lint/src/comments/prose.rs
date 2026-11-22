use super::HISTORY;
use crate::agent_docs::history::{reference_as, Refs};
use crate::rules::{allowed_in, Finding};

/// Where every kept file is the engine's own source, whatever its language.
const ROOTS: &[&str] = &["crates/", "engine/", "games/"];
/// Read whole, line by line: Python, Markdown, a table or a manifest.
const EXTENSIONS: &[&str] = &["py", "md", "txt", "toml"];

/// A kept file under [`ROOTS`] that the code languages' split does not read; `CLAUDE.md`
/// is the agent docs' gate's.
pub(crate) fn read(path: &str) -> bool {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    let name = path.rsplit('/').next().unwrap_or(path);
    ROOTS.iter().any(|r| path.starts_with(r)) && EXTENSIONS.contains(&ext) && name != "CLAUDE.md"
}

pub(super) fn check(path: &str, source: &str) -> Vec<Finding> {
    if !read(path) {
        return Vec::new();
    }
    let lines = source.lines().enumerate();
    lines
        .filter_map(|(i, line)| Some((i, reference_as(line, Refs::CODE)?)))
        .filter(|(i, _)| !allowed_in(path, source, i + 1, HISTORY))
        .map(|(i, what)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: HISTORY,
            message: format!(
                "`{what}` is history; say what the file does: `git blame` finds the issue \
                 and the PR"
            ),
        })
        .collect()
}
