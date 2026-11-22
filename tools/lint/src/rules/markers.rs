use super::finding::Finding;
use super::scan::Scan;
use crate::code::has_word;

pub const RULE: &str = "markers";
/// Spelled in halves so this file does not flag itself.
pub const WORDS: [&str; 4] = [
    concat!("TO", "DO"),
    concat!("FIX", "ME"),
    concat!("XX", "X"),
    concat!("HA", "CK"),
];

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let word = WORDS.iter().find(|w| has_word(l, w))?;
            Some(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: format!("`{word}` marks temporary code; file an issue instead"),
            })
        })
        .collect()
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "markers_tests.rs"]
mod tests;
