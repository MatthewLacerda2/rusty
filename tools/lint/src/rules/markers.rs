//! `markers`: no temporary-code markers in tracked code (#935).
//!
//! Nothing in rusty is temporary: an unfinished thing is an issue, not a comment.
//! A marker word (see [`WORDS`]) standing alone, in upper case, anywhere on a line of
//! a scanned `.rs` file is a finding. Part of a longer word (`TODOS`, `MY_HACK`)
//! is not.

use super::finding::Finding;

pub const RULE: &str = "markers";
/// Spelled in halves so this file does not flag itself.
pub const WORDS: [&str; 4] = [
    concat!("TO", "DO"),
    concat!("FIX", "ME"),
    concat!("XX", "X"),
    concat!("HA", "CK"),
];

/// Every marker in `source`, one finding per line.
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

/// Does `word` occur in `line` with no identifier character on either side?
fn has_word(line: &str, word: &str) -> bool {
    let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    line.match_indices(word).any(|(at, _)| {
        !ident(line[..at].chars().next_back()) && !ident(line[at + word.len()..].chars().next())
    })
}
