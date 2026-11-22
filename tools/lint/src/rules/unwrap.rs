use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "unwrap-deny";
pub const DENY: &str = "#![deny(clippy::unwrap_used)]";
const LINTS: [&str; 2] = ["clippy::unwrap_used", "clippy::restriction"];

pub fn check(path: &str, source: &str, sim_dirs: &[String]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    let Some(dir) = sim_dirs.iter().find(|d| at.starts_with(&format!("{d}/"))) else {
        return Vec::new();
    };
    let finding = |line, message: String| Finding {
        path: path.to_string(),
        line,
        rule: RULE,
        message,
    };
    let stripped = code_lines(source);
    let code: Vec<(usize, &str)> = stripped
        .iter()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .collect();
    let mut out = Vec::new();
    if at == format!("{dir}/mod.rs") && !code.iter().any(|(_, l)| *l == DENY) {
        out.push(finding(
            1,
            format!("a sim module's root must carry `{DENY}`"),
        ));
    }
    for (line, l) in code {
        let lowers = !(l.contains("deny(") || l.contains("forbid("));
        if lowers && LINTS.iter().any(|n| names_lint(l, n)) {
            out.push(finding(
                line,
                "a sim module may not lift the unwrap ban: use `?`, a handled `None`, \
                 or `.expect(\"why\")`"
                    .into(),
            ));
        }
    }
    out
}

pub fn missing_roots(files: &[String], sim_dirs: &[String]) -> Vec<Finding> {
    let roots: Vec<_> = files
        .iter()
        .map(|f| crate::modules::module_path(f))
        .collect();
    sim_dirs
        .iter()
        .map(|d| format!("{d}/mod.rs"))
        .filter(|root| !roots.iter().any(|r| r == root))
        .map(|root| Finding {
            path: root,
            line: 1,
            rule: RULE,
            message: format!("a sim row needs a `mod.rs` root carrying `{DENY}`"),
        })
        .collect()
}

fn names_lint(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let next = code[at + name.len()..].chars().next();
        !next.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = missing_roots(&scan.files, &scan.sim_dirs);
    found.extend(scan.each_file(|p, s| check(p, s, &scan.sim_dirs)));
    found
}

#[cfg(test)]
#[path = "unwrap_tests.rs"]
mod tests;
