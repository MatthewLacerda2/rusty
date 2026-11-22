use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "root-tests";

const MANIFEST: &str = "Cargo.toml";

fn bare(line: &str) -> String {
    let code = line.split('#').next().unwrap_or("");
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

fn needs(header: &str) -> Option<(&'static str, &'static str)> {
    match header {
        "[package]" => Some((
            "autobins=false",
            "add `autobins = false` here: cargo would build a binary it finds under \
             `src/bin/` with a test harness; declare each in a `[[bin]]` instead",
        )),
        "[lib]" | "[[bin]]" => Some((
            "test=false",
            "a root target builds no test harness: add `test = false` here (each \
             harness is one more rustc run and link against the whole engine on every \
             test build, and the root's tests live in `tests/`)",
        )),
        _ => None,
    }
}

pub fn check_manifest(text: &str) -> Vec<Finding> {
    let mut sections: Vec<(usize, &str, &str, bool)> = Vec::new();
    let mut held = false;
    for (i, line) in text.lines().enumerate() {
        let line = bare(line);
        if line.starts_with('[') {
            let need = needs(&line);
            held = need.is_some();
            if let Some((key, message)) = need {
                sections.push((i + 1, key, message, false));
            }
        } else if let Some((_, key, _, said)) = sections.last_mut().filter(|_| held) {
            *said |= line == *key;
        }
    }
    sections
        .into_iter()
        .filter(|(.., said)| !said)
        .map(|(line, _, message, _)| Finding {
            path: MANIFEST.to_string(),
            line,
            rule: RULE,
            message: message.to_string(),
        })
        .collect()
}

pub fn check_source(path: &str, test: &[bool]) -> Option<Finding> {
    // The raw path, not `module_path`, which maps each member's `src/` to `src/<m>/` too.
    if !path.starts_with("src/") {
        return None;
    }
    let at = test.iter().position(|t| *t)?;
    Some(Finding {
        path: path.to_string(),
        line: at + 1,
        rule: RULE,
        message: "the root package builds no unit-test harness (`test = false` in \
                  Cargo.toml), so this test never runs: move it to `tests/` or into the \
                  member crate whose code it tests"
            .to_string(),
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let text = std::fs::read_to_string(scan.base.join(MANIFEST)).unwrap_or_default();
    let mut found = check_manifest(&text);
    found.extend(
        scan.files
            .iter()
            .filter_map(|f| check_source(f, scan.test_lines(f))),
    );
    found
}

#[cfg(test)]
#[path = "root_tests_tests.rs"]
mod tests;
