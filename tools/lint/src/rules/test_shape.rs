use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const INNER_CFG: &str = "test-inner-cfg";
pub const USER_WORKSPACE: &str = "test-user-workspace";
pub const GPU_ENTRY: &str = "test-gpu-entry";

const GPU_HELPER: &str = "src/render/test_gpu";

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut flag = |line: usize, rule: &'static str, message: &str| {
        found.push(Finding {
            path: path.to_string(),
            line,
            rule,
            message: message.to_string(),
        });
    };
    for (i, line) in code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, _)| test[*i])
    {
        let code = line.trim_start();
        if path.starts_with("tests/") && code.starts_with("#![cfg(") {
            flag(
                i + 1,
                INNER_CFG,
                "gate the file at its `mod` line (`#[cfg(feature = \"dev\")] mod …;`), \
                 not with an inner `#![cfg]`",
            );
        }
        if code.contains("in_user_workspace") || code.contains("scenario::run(") {
            flag(
                i + 1,
                USER_WORKSPACE,
                "a test never reads the developer's project; use `scenario::run_isolated`",
            );
        }
        let builds = code.contains("new_headless(") || code.contains("new_headless_with(");
        if !is_helper(path) && builds {
            flag(
                i + 1,
                GPU_ENTRY,
                "ask for a GPU through `crate::render::test_gpu::headless_or_skip` \
                 (or `headless_pair_or_skip`, for two renderers alive at once)",
            );
        }
    }
    found
}

fn is_helper(path: &str) -> bool {
    crate::modules::module_path(path)
        .strip_prefix(GPU_HELPER)
        .is_some_and(|rest| rest == ".rs" || rest.starts_with('/'))
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "test_shape_tests.rs"]
mod tests;
