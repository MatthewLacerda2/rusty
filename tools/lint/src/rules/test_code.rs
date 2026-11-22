use crate::code::code_lines;
use crate::layers::{is_test_gate, item_end};

fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/") || name == "tests.rs" || name.ends_with("_tests.rs")
}

pub fn test_lines(path: &str, source: &str, declared_test_only: bool) -> Vec<bool> {
    let stripped = code_lines(source);
    let lines: Vec<&str> = stripped.iter().map(|l| l.as_ref()).collect();
    let at = crate::modules::module_path(path);
    if !(at.starts_with("src/") || at.starts_with("tests/")) {
        return vec![false; lines.len()];
    }
    if declared_test_only || test_file(path) {
        return vec![true; lines.len()];
    }
    let mut test = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if is_test_gate(&lines[i..]) {
            let end = item_end(&lines, i);
            test[i..=end].fill(true);
            i = end + 1;
        } else {
            i += 1;
        }
    }
    test
}
