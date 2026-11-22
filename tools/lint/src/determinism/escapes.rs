use crate::code::{code_of, Open};

const LOCKED: &[&str] = &["clippy::too_many_lines", "clippy::unwrap_used"];

const ESCAPES: &[(&str, &str)] = &[("src/render/setup/mod.rs", "clippy::too_many_lines")];

pub const ROOTS: &[&str] = &["src", "crates", "tests", "fuzz"];

pub fn violations(path: &str, content: &str) -> Vec<String> {
    let mut sanctioned: Vec<&str> = ESCAPES
        .iter()
        .filter(|(p, _)| *p == crate::modules::module_path(path))
        .map(|(_, lint)| *lint)
        .collect();
    let mut out = Vec::new();
    for (line, attr) in attributes(content) {
        if !(attr.contains("allow(") || attr.contains("expect(")) {
            continue;
        }
        for lint in LOCKED.iter().filter(|l| super::names_lint(&attr, l)) {
            if let Some(i) = sanctioned.iter().position(|s| s == lint) {
                sanctioned.remove(i);
                continue;
            }
            out.push(format!(
                "ESCAPE {path}:{line} `{lint}` — this lint's exceptions are locked; \
                 fix the code instead (docs/linting.md)"
            ));
        }
    }
    out
}

fn attributes(content: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut open: Option<(usize, String, i32)> = None;
    let mut literal = Open::None;
    for (i, raw) in content.lines().enumerate() {
        let mut brackets = 0i32;
        let code = code_of(raw, &mut literal, |_, c| match c {
            '[' => brackets += 1,
            ']' => brackets -= 1,
            _ => {}
        });
        let (start, mut text, mut depth) = match open.take() {
            Some(o) => o,
            None => {
                let trimmed = code.trim_start();
                if !(trimmed.starts_with("#[") || trimmed.starts_with("#![")) {
                    continue;
                }
                (i + 1, String::new(), 0)
            }
        };
        text.push_str(&code);
        depth += brackets;
        if depth > 0 {
            open = Some((start, text, depth));
        } else {
            out.push((start, text));
        }
    }
    out
}
