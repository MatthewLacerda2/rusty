use crate::comments;
use crate::gates::Verdict;
pub(crate) use caps::is_test_file;
use caps::limit_for;
#[cfg(test)]
use caps::{MAX_FILE_LINES, MAX_TEST_FILE_LINES};
use std::fs;
use std::path::{Path, PathBuf};

mod caps;
mod listing;
mod lua;
mod python;
#[cfg(test)]
mod tests;
mod wgsl;

/// The roots whose `.rs` files the rules gate reads (`Scan::at`).
pub(crate) const SCAN_ROOTS: &[&str] = &["src", "crates", "tests", "fuzz", "tools"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    Rust,
    Python,
    Lua,
    Wgsl,
}

impl Lang {
    fn of(path: &Path) -> Option<Lang> {
        match path.extension()?.to_str()? {
            "rs" => Some(Lang::Rust),
            "py" => Some(Lang::Python),
            "lua" => Some(Lang::Lua),
            "wgsl" => Some(Lang::Wgsl),
            _ => None,
        }
    }

    fn measure(self, content: &str) -> usize {
        match self {
            Lang::Rust => content.lines().count(),
            Lang::Python => python::code_lines(content),
            Lang::Lua => lua::code_lines(content),
            Lang::Wgsl => wgsl::code_lines(content),
        }
    }
}

pub fn verdict(base: &Path, args: &[String]) -> Verdict {
    let mut notes = Vec::new();
    let files: Vec<PathBuf> = if args.is_empty() {
        let (files, note) = full_scan(base);
        notes.extend(note);
        files
    } else {
        let paths: Vec<&str> = args.iter().map(String::as_str).collect();
        listing::committable(base, &paths)
            .unwrap_or_else(|_| args.iter().map(|a| base.join(a)).collect())
    };

    let violations: Vec<String> = files.iter().filter_map(|p| check_file(base, p)).collect();
    let listed: Vec<String> = files
        .iter()
        .map(|f| {
            f.strip_prefix(base)
                .unwrap_or(f)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let doc_args = if args.is_empty() { args } else { &listed };
    let (docs, note) = crate::agent_docs::gate(base, doc_args);
    notes.extend(note);
    let comment = comments::pass(base, &files);
    let (over, unshaped, commented) = (violations.len(), docs.len(), comment.len());
    let comment = comment.iter().map(ToString::to_string);
    let findings: Vec<String> = violations.into_iter().chain(docs).chain(comment).collect();
    let mut verdict = Verdict::listed("lint", findings, |_| {
        let mut advice = String::new();
        if over > 0 {
            advice.push_str(&format!(
                "\n{over} file(s) over the size cap. Split them by responsibility; nothing \
                 is grandfathered (see docs/linting.md).\n"
            ));
        }
        if unshaped > 0 {
            advice.push_str(&format!(
                "\n{unshaped} agent-doc finding(s) (`CLAUDE.md`, skill files and \
                 `.claude/agents/` definitions; docs/linting.md, *Agent docs*).\n"
            ));
        }
        if commented > 0 {
            advice.push_str(&format!(
                "\n{commented} comment finding(s) (docs/linting.md, *Comments*).\n"
            ));
        }
        advice
    });
    for note in notes {
        verdict.body.push_str(&note);
        verdict.body.push('\n');
    }
    verdict
}

fn full_scan(base: &Path) -> (Vec<PathBuf>, Option<String>) {
    match listing::committable(base, &[]) {
        Ok(files) => {
            let files = files.into_iter().filter(|p| Lang::of(p).is_some());
            (files.collect(), None)
        }
        Err(why) => {
            let root = if base.as_os_str().is_empty() {
                Path::new(".")
            } else {
                base
            };
            (listing::on_disk(root), Some(fallback_note(&why)))
        }
    }
}

fn fallback_note(why: &str) -> String {
    format!(
        "lint: git can't list the committable files here; measuring every file on disk \
         but the root `.gitignore`'s `/<folder>/` rules, other gitignored ones \
         included\ngit said:\n{why}"
    )
}

pub(crate) fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p) == Some(Lang::Rust));
    out
}

pub(crate) fn shaders_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p) == Some(Lang::Wgsl));
    out
}

#[cfg(test)]
fn measured_in(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out, &|p| Lang::of(p).is_some());
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, wanted: &dyn Fn(&Path) -> bool) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| skipped(&n.to_string_lossy()))
            {
                continue;
            }
            walk(&path, out, wanted);
        } else if wanted(&path) {
            out.push(path);
        }
    }
}

/// Build output, and the hidden folders of tools other than `.github` (`.git`,
/// `.claude/worktrees`), which git would not list either.
fn skipped(dir: &str) -> bool {
    dir == "target" || dir == "__pycache__" || (dir.starts_with('.') && dir != ".github")
}

fn check_file(base: &Path, path: &Path) -> Option<String> {
    let lang = Lang::of(path)?;
    let content = fs::read_to_string(path).ok()?;
    let lines = lang.measure(&content);
    let limit = limit_for(base, path);
    if lines <= limit {
        return None;
    }
    let unit = if lang != Lang::Rust {
        " (lines of code)"
    } else {
        ""
    };
    Some(format!(
        "FILE_TOO_LONG {} {lines}/{limit}{unit}",
        normalize(relative(base, path))
    ))
}

fn relative<'a>(base: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(base).unwrap_or(path)
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().trim_start_matches("./").to_string()
}
