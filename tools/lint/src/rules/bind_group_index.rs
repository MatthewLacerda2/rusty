use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "bind-group-index";
const CALL: &str = "set_bind_group(";

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !crate::modules::module_path(path).starts_with("src/") {
        return Vec::new();
    }
    let lines = bare_lines(source);
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if test.get(i).copied().unwrap_or(false) {
            continue;
        }
        for (at, _) in line.match_indices(CALL) {
            let mut rest = vec![&line[at + CALL.len()..]];
            rest.extend(lines[i + 1..].iter().map(String::as_str));
            if is_number(first_argument(&rest)) {
                found.push(Finding {
                    path: path.to_string(),
                    line: i + 1,
                    rule: RULE,
                    message: "name the group, not its number: a solid-path pass binds \
                              `groups::GLOBAL`, `SCENE`, `MATERIAL` or `DRAW` \
                              (`render::gpu::groups`); any other pass a `const` of its own"
                        .to_string(),
                });
            }
        }
    }
    found
}

fn first_argument<'a>(rest: &[&'a str]) -> &'a str {
    rest.iter()
        .map(|l| l.split(',').next().unwrap_or("").trim())
        .find(|arg| !arg.is_empty())
        .unwrap_or("")
}

fn is_number(arg: &str) -> bool {
    let digits = arg.trim_end_matches("u32").trim_end_matches('_');
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '_')
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "bind_group_index_tests.rs"]
mod tests;
