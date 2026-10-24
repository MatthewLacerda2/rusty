//! `slash-join`: the engine never joins a `/`-separated path onto a path (#1332,
//! #1371).
//!
//! `Path::join("a/b")` keeps the `/` as written, so on Windows it builds `dir\a/b`
//! where joining one part at a time builds `dir\a\b`. The two compare equal as
//! `Path`s but not as strings, and a test comparing strings (a pending set, a
//! written `.mcp.json`, a log line) fails on Windows only. Four Windows runs turned
//! up such a test one at a time (#1263, #1303, #1306, #1331). Production and tests
//! keep one spelling: join one part per call, or join
//! `core::paths::native("a/b")`, which does that for a `/`-spelled path or constant.
//!
//! In `src/` and `tests/`, a `.join(…)`, `.push(…)`, `Path::new(…)` or
//! `PathBuf::from(…)` whose argument is a string literal holding a `/` (raw or
//! not, even wrapped onto the next lines by rustfmt), a `format!` whose template
//! holds one, or a `/`-holding `&str` constant the scan defines (by its name) is a
//! finding. A path made from a `/` path is all-`/` until something joins onto it,
//! then mixed, so it is made with `native` too. A literal of only `/`s and spaces
//! is a slice's separator (`parts.join(" / ")`), not a path, and a literal or
//! constant starting with `/` is an absolute Unix path (a socket, a test's root),
//! not a relative one to respell.
//!
//! Where it misfires, since it reads text, not types: a `.push` of a `/` literal,
//! or of a `format!` whose template has a `/` and no space, onto a `Vec<&str>` or
//! `Vec<String>`; a slice join whose separator holds a `/` beside other text; and
//! a constant matched by its bare name, so a `/` constant in one module flags a
//! same-named plain one joined in another. Where it is blind: a `/` path held in a
//! variable; a `/` constant built by `concat!` or another macro; one whose value
//! rustfmt wrapped onto the line after `const X: &str =`; and one typed
//! `&'static str` or declared `static`. A path that must keep its `/` (a
//! project-relative asset path is `/`-separated by design), or a misfire, says why
//! with `// lint: allow(slash-join) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;
use std::collections::BTreeSet;

pub const RULE: &str = "slash-join";
/// The calls that build a path from their argument, and how each reads it.
const CALLS: [(&str, Call); 4] = [
    (".join(", Call::Join),
    (".push(", Call::Push),
    ("Path::new(", Call::Make),
    ("PathBuf::from(", Call::Make),
];

/// How a call's argument is read. `Push` may be a `Vec<String>`'s, so a pushed
/// `format!` counts only when its template reads as a path (no spaces).
#[derive(Clone, Copy, PartialEq)]
enum Call {
    Join,
    Push,
    Make,
}

/// Every line of `path` where a call joins a `/` path; `slashed` names the
/// `/`-holding constants.
pub fn check_file(path: &str, source: &str, slashed: &BTreeSet<String>) -> Vec<Finding> {
    if !(path.starts_with("src/") || path.starts_with("tests/")) {
        return Vec::new();
    }
    let code = code_lines(source).join("\n");
    let helper = match path.starts_with("tests/") {
        true => "rusty::core::paths::native",
        false => "crate::core::paths::native",
    };
    let mut lines: Vec<(usize, bool)> = CALLS
        .iter()
        .flat_map(|&(name, call)| {
            code.match_indices(name)
                .map(move |(at, _)| (at, name, call))
        })
        .filter(|&(at, name, call)| {
            let arg = code[at + name.len()..].trim_start();
            joins_a_slash(arg, call, slashed)
        })
        .map(|(at, _, call)| (code[..at].matches('\n').count() + 1, call == Call::Make))
        .collect();
    lines.sort_unstable();
    lines.dedup_by_key(|(line, _)| *line);
    lines
        .into_iter()
        .map(|(line, make)| Finding {
            path: path.to_string(),
            line,
            rule: RULE,
            message: match make {
                true => format!(
                    "a path made from a `/` path is mixed-separator on Windows once joined; \
                     make it with `{helper}(\"a/b\")`"
                ),
                false => format!(
                    "a joined `/` path builds a mixed-separator path on Windows; \
                     join one part per call, or join `{helper}(\"a/b\")`"
                ),
            },
        })
        .collect()
}

/// Does the call argument starting `arg` name a `/`-separated path? A literal or
/// constant counts only as the whole argument: `"a/b".into()` is a `String` pushed
/// onto a list.
fn joins_a_slash(arg: &str, call: Call, slashed: &BTreeSet<String>) -> bool {
    let arg = arg.strip_prefix('&').unwrap_or(arg);
    if let Some(template) = arg.strip_prefix("format!(") {
        let spaced = |text: &str| call == Call::Push && text.contains(char::is_whitespace);
        return literal(template.trim_start()).is_some_and(|(t, _)| is_path(t) && !spaced(t));
    }
    if let Some((text, rest)) = literal(arg) {
        return is_path(text) && ends_argument(rest);
    }
    let name: String = arg
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    let last = name.rsplit("::").next().unwrap_or("");
    ends_argument(&arg[name.len()..]) && slashed.contains(last)
}

/// Does the code after an argument close it?
fn ends_argument(rest: &str) -> bool {
    let rest = rest.trim_start();
    rest.starts_with(')') || rest.starts_with(',')
}

/// The text of the string literal (`"…"`, `r"…"` or `r#"…"#`) `code` starts
/// with, and the code after it.
fn literal(code: &str) -> Option<(&str, &str)> {
    if let Some(body) = code.strip_prefix('"') {
        let mut escaped = false;
        let (end, _) = body.char_indices().find(|&(_, c)| {
            let close = c == '"' && !escaped;
            escaped = c == '\\' && !escaped;
            close
        })?;
        return Some((&body[..end], &body[end + 1..]));
    }
    let raw = code.strip_prefix('r')?;
    let hashes = raw.len() - raw.trim_start_matches('#').len();
    let body = raw[hashes..].strip_prefix('"')?;
    let close = format!("\"{}", "#".repeat(hashes));
    let end = body.find(&close)?;
    Some((&body[..end], &body[end + close.len()..]))
}

/// A relative path holds a `/` and a part beside it; `"/"`, `"//"` or `" / "`
/// alone is a slice's separator, and a leading `/` is an absolute Unix path (a
/// socket, a test's out-of-tree root), which has no Windows spelling to match.
fn is_path(text: &str) -> bool {
    let separator = text.chars().all(|c| c == '/' || c.is_whitespace());
    text.contains('/') && !separator && !text.starts_with('/')
}

/// The names of every `&str` constant in `src/` and `tests/` whose value holds a
/// `/` path, or names another such constant (`const A: &str = b::B;`).
pub fn slashed_constants<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
) -> BTreeSet<String> {
    let mut values: Vec<(String, String)> = Vec::new();
    for (path, source) in sources {
        if !(path.starts_with("src/") || path.starts_with("tests/")) {
            continue;
        }
        for line in code_lines(source) {
            let Some((_, decl)) = line.split_once("const ") else {
                continue;
            };
            let Some((name, value)) = decl.split_once(": &str =") else {
                continue;
            };
            let value = value.trim().trim_end_matches(';').trim();
            values.push((name.trim().to_string(), value.to_string()));
        }
    }
    let mut slashed: BTreeSet<String> = values
        .iter()
        .filter(|(_, v)| literal(v).is_some_and(|(t, _)| is_path(t) && !t.contains("://")))
        .map(|(n, _)| n.clone())
        .collect();
    // An alias of an alias is one more pass; stop when a pass adds nothing.
    loop {
        let aliases: Vec<String> = values
            .iter()
            .filter(|(n, v)| {
                !slashed.contains(n) && slashed.contains(v.rsplit("::").next().unwrap_or(""))
            })
            .map(|(n, _)| n.clone())
            .collect();
        if aliases.is_empty() {
            return slashed;
        }
        slashed.extend(aliases);
    }
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    let slashed = slashed_constants(scan.files.iter().map(|p| (p.as_str(), scan.source(p))));
    scan.each_file(|p, s| check_file(p, s, &slashed))
}

#[cfg(test)]
#[path = "slash_join_tests.rs"]
mod tests;
