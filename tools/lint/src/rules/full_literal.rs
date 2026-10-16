//! `full-literal`: a test builds a first-class component through a constructor or
//! `..Default::default()`, never a literal naming every field (#663, #976).
//!
//! A literal that spells out every field breaks the day the component gains one,
//! so one new field means a sweep across the suite, and a reader can't see which
//! field the test is about. The rule is scoped to the first-class components (the
//! `…Component` types `Entity` names in `src/components/entity/mod.rs`): any struct
//! expression in test code (`test_code.rs`) that names every field of one with
//! three or more, with no `..rest`, is a finding. When every field is the point of the test, say so:
//! `// lint: allow(full-literal) <why>`.

use super::finding::Finding;
use super::test_code;
use std::collections::BTreeMap;

pub const RULE: &str = "full-literal";
const ENTITY: &str = "src/components/entity/mod.rs";
/// A component with fewer fields is exempt: `MaterialComponent { name }` is the
/// clearest way to build a one-field struct, and a new field there breaks few tests.
const MIN_FIELDS: usize = 3;

/// Each first-class component's name and its named fields.
pub struct Components(BTreeMap<String, Vec<String>>);

impl Components {
    /// The components `Entity` names, with the fields of their `pub struct` in
    /// `sources` (path, text). A component under [`MIN_FIELDS`] fields is left out.
    pub fn load<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let sources: Vec<_> = sources.into_iter().collect();
        let entity = sources
            .iter()
            .find(|(p, _)| *p == ENTITY)
            .map_or("", |s| s.1);
        let mut map = BTreeMap::new();
        for name in component_types(entity) {
            if let Some(fields) = sources.iter().find_map(|(_, s)| struct_fields(s, &name)) {
                if fields.len() >= MIN_FIELDS {
                    map.insert(name, fields);
                }
            }
        }
        Components(map)
    }

    /// Every full literal of a component in the test code of `path`.
    pub fn check(&self, path: &str, source: &str) -> Vec<Finding> {
        let Some(first) = test_code::first_line(path, source) else {
            return Vec::new();
        };
        let lines: Vec<&str> = source.lines().map(strip_comment).collect();
        let mut found = Vec::new();
        for (i, line) in lines.iter().enumerate().skip(first) {
            for (name, fields) in &self.0 {
                for at in literal_starts(line, name) {
                    let named = literal_fields(&lines[i..], &line[at..]);
                    if named.is_some_and(|n| fields.iter().all(|f| n.contains(f))) {
                        found.push(Finding {
                            path: path.to_string(),
                            line: i + 1,
                            rule: RULE,
                            message: format!(
                                "`{name}` names all {} fields; build it with \
                                 `..Default::default()` or a constructor",
                                fields.len()
                            ),
                        });
                    }
                }
            }
        }
        found
    }
}

/// The `…Component` type names in the body of `pub struct Entity`.
fn component_types(entity: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in body(entity, "Entity")
        .unwrap_or_default()
        .split(|c: char| !is_ident(c))
    {
        if word.ends_with("Component") && !out.iter().any(|w| w == word) {
            out.push(word.to_string());
        }
    }
    out
}

/// The named fields of `pub struct name { … }` in `source`, if it is defined there.
fn struct_fields(source: &str, name: &str) -> Option<Vec<String>> {
    let fields: Vec<String> = body(source, name)?
        .lines()
        .map(|l| strip_comment(l).trim())
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let l = l
                .strip_prefix("pub(crate) ")
                .or(l.strip_prefix("pub "))
                .unwrap_or(l);
            let (field, _) = l.split_once(':')?;
            field.chars().all(is_ident).then(|| field.to_string())
        })
        .collect();
    (!fields.is_empty()).then_some(fields)
}

/// The text between the braces of `pub struct name {`, joined by newlines.
fn body(source: &str, name: &str) -> Option<String> {
    let head = format!("pub struct {name} {{");
    let mut lines = source
        .lines()
        .skip_while(|l| !l.trim_start().starts_with(&head));
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with('}')).collect();
    Some(body.join("\n"))
}

/// Byte offsets in `line` where a struct expression `name {` starts: the whole
/// identifier, not a `struct`/`impl` header.
fn literal_starts(line: &str, name: &str) -> Vec<usize> {
    line.match_indices(name)
        .filter(|(at, _)| {
            let before = line[..*at].chars().next_back();
            let after = line[at + name.len()..].trim_start();
            let lead = line[..*at].trim_end();
            !before.is_some_and(is_ident)
                && after.starts_with('{')
                && !["struct", "for", "->"].iter().any(|k| lead.ends_with(k))
                && !lead.trim_start().starts_with("impl")
        })
        .map(|(at, _)| at)
        .collect()
}

/// The field names a literal starting at `from` (the rest of the first line, then
/// `rest[1..]`) sets at its top level, or `None` when it has a `..rest`.
fn literal_fields(rest: &[&str], from: &str) -> Option<Vec<String>> {
    let text = std::iter::once(from)
        .chain(rest.iter().skip(1).copied())
        .collect::<Vec<_>>()
        .join("\n");
    let open = text.find('{')?;
    let (mut depth, mut in_str) = (0i32, false);
    let (mut item, mut items) = (String::new(), Vec::new());
    for c in text[open + 1..].chars() {
        if c == '"' {
            in_str = !in_str;
        } else if !in_str {
            match c {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' if depth == 0 => break,
                '}' | ')' | ']' => depth -= 1,
                ',' if depth == 0 => {
                    items.push(std::mem::take(&mut item));
                    continue;
                }
                _ => {}
            }
        }
        item.push(c);
    }
    items.push(item);
    let mut names = Vec::new();
    for item in items.iter().map(|i| i.trim()).filter(|i| !i.is_empty()) {
        if item.starts_with("..") {
            return None;
        }
        let name = item.split(':').next().unwrap_or(item).trim();
        names.push(name.to_string());
    }
    Some(names)
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Drop a trailing `//` comment (not string-aware; a coarse source scan).
fn strip_comment(line: &str) -> &str {
    line.find("//").map_or(line, |i| &line[..i])
}
