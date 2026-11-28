//! Every `ui::` row of the component registry and every script in `engine/scripts/ui/` is
//! used by a test game. A component is used when a game's code names it as a string or calls
//! its namespace; a widget, when a string names it as `UI.Create`, a prefab path or
//! `Scene.GetScript` does. Only what a game runs counts: `recipe.lua`, `scripts/` and
//! `games/shared/`, comments cut (`docs/linting.md`, `ui-coverage`).

use super::finding::{allowed_anywhere, allowed_in, Finding};
use super::scan::Scan;
use crate::comments::{split, Kind};
use std::fs;
use std::path::{Path, PathBuf};

pub const RULE: &str = "ui-coverage";

const REGISTRY: &str = "crates/components/src/registry.rs";
const DECL: &str = "macro_rules! first_class {";
const WIDGETS: &str = "engine/scripts/ui";
const GAMES: &str = "games";
const SHARED: &str = "shared";

pub fn run(scan: &Scan) -> Vec<Finding> {
    check(scan.root())
}

pub fn check(base: &Path) -> Vec<Finding> {
    let games = base.join(GAMES);
    if !games.is_dir() {
        return Vec::new();
    }
    let code = game_code(&games);
    let mut found = Vec::new();
    let registry = fs::read_to_string(base.join(REGISTRY)).unwrap_or_default();
    let components = components(&registry);
    if components.is_empty() {
        found.push(lost(
            REGISTRY,
            "no `ui::` row in its `first_class!` registry",
        ));
    }
    for (line, kind) in components {
        if !uses_component(&code, &kind) && !allowed_in(REGISTRY, &registry, line, RULE) {
            found.push(unused(
                REGISTRY,
                line,
                &format!("UI component `{kind}`"),
                &kind,
            ));
        }
    }
    let widgets = widgets(&base.join(WIDGETS));
    if widgets.is_empty() {
        found.push(lost(WIDGETS, "no widget script in it"));
    }
    for stem in widgets {
        let path = format!("{WIDGETS}/{stem}.lua");
        let source = fs::read_to_string(base.join(&path)).unwrap_or_default();
        if !uses_widget(&code, &stem) && !allowed_anywhere(&path, &source, RULE) {
            found.push(unused(&path, 1, &format!("widget script `{stem}`"), &stem));
        }
    }
    found
}

fn lost(path: &str, what: &str) -> Finding {
    Finding {
        path: path.to_string(),
        line: 1,
        rule: RULE,
        message: format!(
            "`games/` exists but this check found {what}, so it would pass on nothing; \
             point `tools/lint/src/rules/ui_coverage.rs` at where the UI parts moved"
        ),
    }
}

fn unused(path: &str, line: usize, what: &str, name: &str) -> Finding {
    Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message: format!(
            "no test game uses the {what}: no recipe or script under `games/` names `{name}`; \
             give it to the game it suits (`docs/testing.md`, *Test games*), or excuse it \
             here with `lint: allow({RULE}) <why>`"
        ),
    }
}

/// `(line, Kind)` of each registry row whose type path starts `ui::`.
fn components(registry: &str) -> Vec<(usize, String)> {
    let Some(start) = registry.find(DECL) else {
        return Vec::new();
    };
    let first = registry[..start].lines().count();
    registry[start..]
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let (_, rest) = line.trim().split_once(": ")?;
            let (path, rest) = rest.split_once(" => ")?;
            let (kind, _) = rest.split_once(" [")?;
            path.starts_with("ui::")
                .then(|| (first + i + 1, kind.to_string()))
        })
        .collect()
}

fn widgets(dir: &Path) -> Vec<String> {
    let mut stems: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "lua"))
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
        .collect();
    stems.sort();
    stems
}

fn game_code(games: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(games)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        if dir.file_name().is_some_and(|n| n == SHARED) {
            lua_files(&dir, &mut files);
        } else {
            files.push(dir.join("recipe.lua"));
            lua_files(&dir.join("scripts"), &mut files);
        }
    }
    files
        .iter()
        .filter_map(|f| fs::read_to_string(f).ok())
        .flat_map(|text| split(Kind::Lua, &text).into_iter().map(|l| l.code))
        .collect()
}

fn lua_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            lua_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "lua") {
            out.push(p);
        }
    }
}

fn uses_component(code: &[String], kind: &str) -> bool {
    code.iter()
        .any(|line| strings(line).any(|s| s == kind) || calls(line, kind))
}

/// `Kind.` as a whole name: not the tail of `Other.Kind.` or `MyKind.`.
fn calls(line: &str, kind: &str) -> bool {
    let call = format!("{kind}.");
    line.match_indices(&call).any(|(at, _)| {
        line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'))
    })
}

fn uses_widget(code: &[String], stem: &str) -> bool {
    let want = squash(stem);
    code.iter()
        .any(|line| strings(line).any(|s| squash(named(s)) == want))
}

fn named(s: &str) -> &str {
    let file = s.rsplit('/').next().unwrap_or(s);
    [".lua", ".prefab"]
        .iter()
        .find_map(|ext| file.strip_suffix(ext))
        .unwrap_or(file)
}

/// `"Scroll View"`, `"scroll_view"` and `"ScrollView"` all read `scrollview`.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| *c != ' ' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn strings(line: &str) -> impl Iterator<Item = &str> {
    let mut rest = line;
    std::iter::from_fn(move || {
        let open = rest.find(['"', '\''])?;
        let quote = rest[open..].chars().next()?;
        let body = &rest[open + 1..];
        let mut escaped = false;
        let close = body.char_indices().find_map(|(i, c)| {
            let end = !escaped && c == quote;
            escaped = !escaped && c == '\\';
            end.then_some(i)
        });
        match close {
            Some(end) => {
                rest = &body[end + 1..];
                Some(&body[..end])
            }
            None => {
                rest = "";
                None
            }
        }
    })
}

#[cfg(test)]
#[path = "ui_coverage_tests.rs"]
mod tests;
