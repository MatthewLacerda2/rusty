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

#[derive(Clone, Copy)]
pub(crate) struct Refs {
    pub min_digits: usize,
    pub foreign_pass: bool,
    /// `#` and six or eight digits is a colour (`#202020`), no issue.
    pub digit_colours: bool,
}

impl Refs {
    pub const AGENT_DOCS: Refs = Refs {
        min_digits: 1,
        foreign_pass: false,
        digit_colours: false,
    };
    pub const CODE: Refs = Refs {
        min_digits: 3,
        foreign_pass: true,
        digit_colours: true,
    };
}

const OWN: &[&str] = &["rusty", "pr", "prs", "issue", "issues", "pull"];

fn reference(line: &str) -> Option<String> {
    reference_as(line, Refs::AGENT_DOCS)
}

pub(crate) fn reference_as(line: &str, refs: Refs) -> Option<String> {
    let b = line.as_bytes();
    let number = |from: usize, min: usize| {
        let end = (from..b.len())
            .find(|&j| !b[j].is_ascii_digit())
            .unwrap_or(b.len());
        let open = b.get(from).is_some_and(|c| (b'1'..=b'9').contains(c));
        let closed = b
            .get(end)
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        (open && closed && end - from >= min).then_some(end)
    };
    let foreign = |repo: &str| {
        let name = repo.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        refs.foreign_pass && !name.is_empty() && !OWN.contains(&name.as_str())
    };
    for i in 0..b.len() {
        let anchor = i >= 2 && &b[i - 2..i] == b"](";
        if b[i] == b'#' && (i == 0 || b[i - 1] != b'&') && !anchor {
            let named = named(&line[..i]);
            let min = if named { 1 } else { refs.min_digits };
            let colour = |end: usize| refs.digit_colours && !named && [7, 9].contains(&(end - i));
            let found = number(i + 1, min).filter(|&end| !colour(end));
            if let Some(end) = found.filter(|_| !foreign(repo_before(&line[..i]))) {
                return Some(line[i..end].to_string());
            }
        }
        for seg in ["/issues/", "/pull/"] {
            if b[i..].starts_with(seg.as_bytes()) {
                if let Some(end) =
                    number(i + seg.len(), refs.min_digits).filter(|_| !foreign(&line[..i]))
                {
                    return Some(line[i..end].to_string());
                }
            }
        }
    }
    None
}

/// `issue #` or `PR #` then any number: a reference by its own words, however short.
fn named(before: &str) -> bool {
    let word = before.strip_suffix(' ').unwrap_or(before);
    let word = word
        .rsplit(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("");
    ["issue", "issues", "pr", "prs"].contains(&word.to_ascii_lowercase().as_str())
}

fn repo_before(text: &str) -> &str {
    let glued = |c: char| c.is_ascii_alphanumeric() || "_-./".contains(c);
    let start = text.trim_end_matches(glued).len();
    &text[start..]
}

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
