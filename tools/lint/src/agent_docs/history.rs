//! What an agent doc must not carry: its history. An issue or pull request number
//! (`#123`, `owner/repo#123`, `…/issues/123`, `…/pull/123`) or a date
//! (`YYYY-MM-DD`) tells the story behind a rule; the rule is what the agent needs,
//! and `git log -S` and the issue tracker find the story.
//!
//! Look-alikes pass: a placeholder (`#N`), a hex colour (`#33ccff`, or `#0f0`,
//! since no issue number opens with `0`), an HTML entity (`&#123;`), a version
//! string (`1.95.0`) and a digit run that is no calendar date (`1234-56-78`). A
//! colour of only digits (`#333333`) reads as a number: write it lowercase-hex or
//! take an allow.

/// Every line of `source` that carries history, as (1-based line, message), one
/// per line, naming its first such reference.
pub fn check(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let what = reference(l).or_else(|| date(l))?;
            Some((
                i + 1,
                format!(
                    "`{what}` is history; state the rule: its reasoning lives in the issue \
                     and PR (`git log -S` finds them)"
                ),
            ))
        })
        .collect()
}

/// The first issue or pull request reference on `line`.
fn reference(line: &str) -> Option<String> {
    let b = line.as_bytes();
    for i in 0..b.len() {
        let number = |from: usize| {
            let end = (from..b.len())
                .find(|&j| !b[j].is_ascii_digit())
                .unwrap_or(b.len());
            let open = b.get(from).is_some_and(|c| (b'1'..=b'9').contains(c));
            let closed = b
                .get(end)
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
            (open && closed).then_some(end)
        };
        if b[i] == b'#' && (i == 0 || b[i - 1] != b'&') {
            if let Some(end) = number(i + 1) {
                return Some(line[i..end].to_string());
            }
        }
        for seg in ["/issues/", "/pull/"] {
            if b[i..].starts_with(seg.as_bytes()) {
                if let Some(end) = number(i + seg.len()) {
                    return Some(line[i..end].to_string());
                }
            }
        }
    }
    None
}

/// The first `YYYY-MM-DD` calendar date on `line`, not part of a longer run.
fn date(line: &str) -> Option<String> {
    let b = line.as_bytes();
    let digits = |r: std::ops::Range<usize>| r.clone().all(|j| b[j].is_ascii_digit());
    let num = |r: std::ops::Range<usize>| line[r].parse::<u32>().unwrap_or(0);
    (0..b.len().saturating_sub(9)).find_map(|i| {
        let shape = digits(i..i + 4)
            && b[i + 4] == b'-'
            && digits(i + 5..i + 7)
            && b[i + 7] == b'-'
            && digits(i + 8..i + 10);
        let alone = (i == 0 || !b[i - 1].is_ascii_alphanumeric())
            && b.get(i + 10).is_none_or(|c| !c.is_ascii_digit());
        let calendar =
            || (1..=12).contains(&num(i + 5..i + 7)) && (1..=31).contains(&num(i + 8..i + 10));
        (shape && alone && calendar()).then(|| line[i..i + 10].to_string())
    })
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
