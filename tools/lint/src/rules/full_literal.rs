use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_lines, is_ident};
use std::collections::BTreeMap;

pub const RULE: &str = "full-literal";
const ENTITY: [&str; 2] = ["src/components/entity/mod.rs", "src/components/registry.rs"];
/// Fields: below this a literal is the clearest build, and a new field breaks few tests.
const MIN_FIELDS: usize = 3;

pub struct Components(BTreeMap<String, Vec<String>>);

impl Components {
    pub fn load<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let sources: Vec<_> = sources.into_iter().collect();
        let mut names = Vec::new();
        for (path, text) in &sources {
            if ENTITY.contains(&&*crate::modules::module_path(path)) {
                names.extend(component_types(text));
            }
        }
        let mut map = BTreeMap::new();
        for name in names {
            if let Some(fields) = sources.iter().find_map(|(_, s)| struct_fields(s, &name)) {
                if fields.len() >= MIN_FIELDS {
                    map.insert(name, fields);
                }
            }
        }
        Components(map)
    }

    pub fn check(&self, path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
        let stripped = code_lines(source);
        let lines: Vec<&str> = stripped.iter().map(|l| l.as_ref()).collect();
        let mut found = Vec::new();
        for (i, line) in lines.iter().enumerate().filter(|(i, _)| test[*i]) {
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

fn component_types(source: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let rows = source
        .split_once("macro_rules! first_class {")
        .map(|(_, r)| r.to_string());
    for word in rows
        .or_else(|| body(source, "Entity"))
        .unwrap_or_default()
        .split(|c: char| !is_ident(c))
    {
        if word.ends_with("Component") && !out.iter().any(|w| w == word) {
            out.push(word.to_string());
        }
    }
    out
}

fn struct_fields(source: &str, name: &str) -> Option<Vec<String>> {
    let fields: Vec<String> = code_lines(&body(source, name)?)
        .iter()
        .map(|l| l.trim())
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

fn body(source: &str, name: &str) -> Option<String> {
    let head = format!("pub struct {name} {{");
    let mut lines = source
        .lines()
        .skip_while(|l| !l.trim_start().starts_with(&head));
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with('}')).collect();
    Some(body.join("\n"))
}

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

pub fn run(scan: &Scan) -> Vec<Finding> {
    let components = Components::load(scan.files.iter().map(|p| (p.as_str(), scan.source(p))));
    scan.each_file(|p, s| components.check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "full_literal_tests.rs"]
mod tests;

#[cfg(test)]
#[test]
/// `Entity`'s body alone names only Transform: its other fields come from the registry.
fn the_real_tree_names_the_registry_components() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scan = Scan::at(&root);
    let found = Components::load(scan.files.iter().map(|p| (p.as_str(), scan.source(p))));
    for name in [
        "TransformComponent",
        "LightComponent",
        "NavMeshModifierComponent",
    ] {
        assert!(found.0.contains_key(name), "`{name}` not found");
    }
}
