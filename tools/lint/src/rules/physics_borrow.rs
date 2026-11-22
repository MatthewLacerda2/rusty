use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "physics-borrow";

const SETS: &[&str] = &["bodies", "colliders"];
const BORROWS: &[&str] = &[".get_mut(", ".iter_mut("];

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/physics/") {
        return Vec::new();
    }
    let lines = code_lines(source);
    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !test.get(*i).copied().unwrap_or(false))
        .filter(|&(i, code)| raw_borrow(code, i.checked_sub(1).map(|p| lines[p].as_ref())))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a raw mutable borrow marks the body or collider modified: borrow \
                      through `body_to_write` / `collider_to_write` (`physics::writes`)"
                .to_string(),
        })
        .collect()
}

fn raw_borrow(code: &str, above: Option<&str>) -> bool {
    let wrapped = above.is_some_and(|a| SETS.iter().any(|s| ends_with_word(a.trim_end(), s)))
        && BORROWS.iter().any(|b| code.trim_start().starts_with(b));
    wrapped || SETS.iter().any(|set| set_borrow(code, set))
}

fn set_borrow(code: &str, set: &str) -> bool {
    word_at(code, set).any(|at| {
        let rest = &code[at + set.len()..];
        if BORROWS.iter().any(|b| rest.starts_with(b)) {
            return true;
        }
        let Some(index) = rest.strip_prefix('[') else {
            return false;
        };
        let after = index
            .find(']')
            .map_or("", |end| index[end + 1..].trim_start());
        let assigned = after.starts_with('=') && !after.starts_with("==");
        code[..at].contains("&mut") || after.starts_with(".set_") || assigned
    })
}

fn word_at<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word)
        .map(|(at, _)| at)
        .filter(move |&at| {
            !ident(code[..at].chars().next_back()) && !ident(code[at + word.len()..].chars().next())
        })
}

fn ends_with_word(code: &str, word: &str) -> bool {
    code.strip_suffix(word)
        .is_some_and(|head| !ident(head.chars().next_back()))
}

fn ident(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "physics_borrow_tests.rs"]
mod tests;
