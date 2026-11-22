use super::File;
use crate::code::{is_ident, ITEMS};
use crate::comments::{declared, split, Kind, Line};

const QUALIFIERS: &[&str] = &["unsafe", "async", "extern", "pub", "crate", "in", "super"];

pub struct Claim<'a> {
    pub name: &'a str,
    /// `mod` for a hook module.
    pub kind: &'a str,
    pub parent: String,
    /// The type of the `impl` an associated item sits in.
    pub owner: Option<&'a str>,
    /// A module's path in its crate (`occlusion::test_gap`), for a module of hooks.
    pub module: Option<String>,
    pub line: usize,
    pub text: String,
}

/// `a::b` for `crates/<m>/src/a/b.rs` or `…/a/b/mod.rs`; `""` for the crate root.
pub fn module_path(path: &str) -> Option<String> {
    let inner = path.split_once("/src/")?.1.strip_suffix(".rs")?;
    let inner = inner.strip_suffix("/mod").unwrap_or(inner);
    Some(match inner {
        "lib" | "main" | "mod" => String::new(),
        _ => inner.replace('/', "::"),
    })
}

fn module_name(path: &str) -> Option<&str> {
    let (dir, file) = path.rsplit_once('/')?;
    match file {
        "lib.rs" | "main.rs" => None,
        "mod.rs" => dir.rsplit('/').next(),
        _ => file.strip_suffix(".rs"),
    }
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}::{name}")
    }
}

/// Where `text` says "test hook(s)" as words (`crates/CLAUDE.md`, *Test hooks*).
pub fn phrase(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let touches = |c: Option<char>| c.is_some_and(is_ident);
    lower
        .match_indices("test hook")
        .map(|(at, _)| at)
        .find(|&at| {
            let after = lower[at + "test hook".len()..].trim_start_matches('s');
            !touches(lower[..at].chars().next_back()) && !touches(after.chars().next())
        })
}

pub fn claims<'a>(file: &File<'a>) -> Vec<Claim<'a>> {
    let lines = split(Kind::Rust, file.source);
    let note = |l: &Line| l.code.trim().is_empty() && !l.comment.trim().is_empty();
    let here = module_path(file.path).unwrap_or_default();
    let mut out = Vec::new();
    let mut rest = &lines[..];
    while let Some(first) = rest.first() {
        let i = lines.len() - rest.len();
        let run = rest.iter().take_while(|l| note(l)).count();
        if run == 0 {
            rest = &rest[1..];
            continue;
        }
        let (block, after) = rest.split_at(run);
        rest = after;
        let text: Vec<&str> = block.iter().map(|l| l.comment.as_str()).collect();
        let text = text.join(" ");
        let tested = file.test.get(i).copied().unwrap_or(false);
        let Some(at) = phrase(&text).filter(|_| !tested) else {
            continue;
        };
        let found = if first.comment.trim_start().starts_with("//!") {
            header(file, &text, at, &here)
        } else {
            let item = item_after(file.source, i + run);
            item.into_iter()
                .map(|(kind, name, owner)| {
                    let module = (kind == "mod").then(|| join(&here, name));
                    (kind, name, owner, module)
                })
                .collect()
        };
        out.extend(found.into_iter().map(|(kind, name, owner, module)| Claim {
            name,
            kind,
            parent: here.clone(),
            owner,
            module,
            line: i + 1,
            text: text.clone(),
        }));
    }
    out
}

type Found<'a> = (&'a str, &'a str, Option<&'a str>, Option<String>);

/// The items a `//!` header's sentence names, else its module if the sentence opens on
/// the phrase.
fn header<'a>(file: &File<'a>, text: &str, at: usize, here: &str) -> Vec<Found<'a>> {
    let start = text[..at].rfind(". ").map_or(0, |dot| dot + 2);
    let end = text[at..].find(". ").map_or(text.len(), |dot| at + dot);
    let sentence = &text[start..end];
    let items: Vec<(&str, &str)> = file
        .source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .flat_map(declared)
        .filter(|(kind, _)| *kind != "impl")
        .collect();
    let spans = sentence.split('`').skip(1).step_by(2);
    let named: Vec<Found> = spans
        .filter_map(|span| items.iter().find(|(_, name)| *name == span))
        .map(|&(kind, name)| (kind, name, None, None))
        .collect();
    if !named.is_empty() {
        return named;
    }
    let opening = sentence.trim_start_matches(['/', '!', ' ']);
    let opening = opening.strip_prefix("A ").unwrap_or(opening);
    let module_is_one = phrase(opening) == Some(0);
    match module_name(file.path).filter(|_| module_is_one) {
        Some(name) => vec![("mod", name, None, Some(here.to_string()))],
        None => Vec::new(),
    }
}

/// Keyword, name and `impl` type of the item below; none when `#[cfg(test)]`.
fn item_after(source: &str, from: usize) -> Option<(&str, &str, Option<&str>)> {
    let lines: Vec<&str> = source.lines().collect();
    let at = (from..lines.len()).find(|&i| {
        let l = lines[i].trim();
        !l.starts_with("#[") && !l.starts_with("//")
    })?;
    if lines[from..at].iter().any(|l| l.contains("cfg(test)")) {
        return None;
    }
    let mut tokens = lines[at]
        .split(|c: char| !is_ident(c))
        .filter(|t| !t.is_empty());
    let kind = tokens
        .find(|t| ITEMS.contains(t))
        .filter(|k| *k != "impl")?;
    let name = tokens.find(|t| !ITEMS.contains(t) && !QUALIFIERS.contains(t))?;
    let indented = lines[at].starts_with(char::is_whitespace);
    let owner = lines[..at]
        .iter()
        .rev()
        .find(|l| l.starts_with("impl"))
        .filter(|_| indented)
        .and_then(|l| impl_type(l));
    Some((kind, name, owner))
}

/// `Type` of `impl<T> Trait for Type<T>`.
fn impl_type(line: &str) -> Option<&str> {
    let line = line.strip_prefix("impl")?;
    let line = line.rsplit_once(" for ").map_or(line, |(_, t)| t);
    let mut depth = 0i32;
    let start = line.find(|c: char| {
        depth += match c {
            '<' => 1,
            '>' => -1,
            _ => 0,
        };
        depth == 0 && is_ident(c)
    })?;
    let rest = &line[start..];
    Some(&rest[..rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len())])
}

/// The crates a hook line says call it: a backticked crate (`app`, `rusty-dev`,
/// `dev::bake`), and `""` for the root package's tests.
pub fn named<'a>(
    text: &str,
    own: &str,
    crates: &'a [String],
    own_module: &dyn Fn(&str) -> bool,
) -> Vec<&'a str> {
    let text = phrase(text).map_or(text, |at| &text[at..]);
    let mut out: Vec<&str> = Vec::new();
    for span in text.split('`').skip(1).step_by(2) {
        let bare = span.strip_prefix("rusty-").or(span.strip_prefix("rusty_"));
        let head = bare.unwrap_or(span).split([':', '/', '\'']).next();
        let head = head.unwrap_or("").replace('_', "-");
        let theirs = crates.iter().find(|c| **c == head && c.as_str() != own);
        if let Some(m) = theirs.filter(|_| bare.is_some() || !own_module(&head)) {
            out.push(m);
        }
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("the root's") || lower.contains("root package") {
        out.push("");
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
#[path = "claims_tests.rs"]
mod tests;
