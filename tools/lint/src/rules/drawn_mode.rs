use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "drawn-mode";

const SCOPE: &str = "src/render/";

const ENTRY: &str = "src/render/clusters/unbaked.rs";

const READS: [&str; 2] = ["light_mode(", "effective_mode("];

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with(SCOPE) || at == ENTRY {
        return Vec::new();
    }
    let is_test = |i: usize| test.get(i).copied().unwrap_or(false);
    (code_lines(source).iter().enumerate())
        .filter(|&(i, line)| !is_test(i) && reads_mode(line.as_ref()))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "reads a light's mode past what the frame draws it as: read \
                      `crate::clusters::drawn_mode` (it previews a `Baked` light no bake \
                      holds yet)"
                .to_string(),
        })
        .collect()
}

fn reads_mode(code: &str) -> bool {
    READS.iter().any(|read| {
        code.match_indices(read).any(|(at, _)| {
            !(code[..at].chars().next_back()).is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "drawn_mode_tests.rs"]
mod tests;
