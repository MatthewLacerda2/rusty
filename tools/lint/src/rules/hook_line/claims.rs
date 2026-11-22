use super::File;
use crate::code::is_ident;
use crate::comments::{split, Kind, Line};

const ITEMS: &[&str] = &[
    "fn",
    "struct",
    "enum",
    "trait",
    "mod",
    "const",
    "static",
    "type",
    "macro_rules",
];
const QUALIFIERS: &[&str] = &["unsafe", "async", "extern", "pub", "crate", "in", "super"];

pub struct Claim<'a> {
    pub name: &'a str,
    /// `mod` for a module of hooks.
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

pub fn claims<'a>(file: &File<'a>) -> Vec<Claim<'a>> {
    let lines = split(Kind::Rust, file.source);
    let note = |l: &Line| l.code.trim().is_empty() && !l.comment.trim().is_empty();
    let here = module_path(file.path).unwrap_or_default();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let run = lines[i..].iter().take_while(|l| note(l)).count();
        if run == 0 {
            i += 1;
            continue;
        }
        let block = &lines[i..i + run];
        let text: Vec<&str> = block.iter().map(|l| l.comment.as_str()).collect();
        let text = text.join(" ");
        let tested = file.test.get(i).copied().unwrap_or(false);
        if !tested && text.to_ascii_lowercase().contains("test hook") {
            let header = block[0].comment.trim_start().starts_with("//!");
            let claim = if header {
                let name = module_name(file.path);
                name.map(|name| ("mod", name, None, Some(here.clone())))
            } else {
                item_after(file.source, i + run).map(|(kind, name, owner)| {
                    let module = (kind == "mod").then(|| join(&here, name));
                    (kind, name, owner, module)
                })
            };
            out.extend(claim.map(|(kind, name, owner, module)| Claim {
                name,
                kind,
                parent: here.clone(),
                owner,
                module,
                line: i + 1,
                text,
            }));
        }
        i += run;
    }
    out
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
    let kind = tokens.find(|t| ITEMS.contains(t))?;
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

/// `Type` of `impl<T> Trait for Type<T> {`.
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
    let lower = text.to_ascii_lowercase();
    let text = lower.find("test hook").map_or(text, |at| &text[at..]);
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
