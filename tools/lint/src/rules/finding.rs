//! The one report shape and the one inline exception every rule shares (#934).
//!
//! A rule reports a [`Finding`] at `path:line`. Code that breaks a rule on purpose
//! says why on that line, or in the comment and attribute lines right above it:
//!
//! ```text
//! // lint: allow(prefix-group) one file per Unity component, mirrored in the editor
//! ```
//!
//! The reason is required: an allow without one is itself a finding, under the
//! [`ALLOW`] rule. Rules never grow their own flag or baseline for exceptions.

use std::fmt;

/// The rule an allow comment with no reason is reported under.
pub const ALLOW: &str = "allow";
const MARKER: &str = "lint: allow(";

/// One broken rule, at one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Repo-relative path, `/`-separated.
    pub path: String,
    /// 1-based line the finding (and any allow for it) anchors to.
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: [{}] {}",
            self.path, self.line, self.rule, self.message
        )
    }
}

/// An allow comment on `line`: the rule it names, and its reason (maybe empty).
pub(super) fn parse_allow(line: &str) -> Option<(&str, &str)> {
    // comment-cut: this reads the comment itself, never the code before it
    let comment = &line[line.find("//")?..];
    let rest = &comment[comment.find(MARKER)? + MARKER.len()..];
    let close = rest.find(')')?;
    Some((rest[..close].trim(), rest[close + 1..].trim()))
}

/// Is the 1-based `line` of `source` excused from `rule`? An allow counts on the
/// line itself or in the unbroken run of comment and attribute lines above it,
/// and only when it gives a reason.
pub fn allowed(source: &str, line: usize, rule: &str) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let Some(at) = line.checked_sub(1).filter(|&i| i < lines.len()) else {
        return false;
    };
    let excuses = |l: &str| parse_allow(l).is_some_and(|(r, why)| r == rule && !why.is_empty());
    if excuses(lines[at]) {
        return true;
    }
    lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim_start())
        .take_while(|l| l.starts_with("//") || l.starts_with("#["))
        .any(excuses)
}

/// Every allow in `source` that names a rule but gives no reason.
pub fn unexplained(path: &str, source: &str) -> Vec<Finding> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let (rule, why) = parse_allow(l)?;
            why.is_empty().then(|| Finding {
                path: path.to_string(),
                line: i + 1,
                rule: ALLOW,
                message: format!("`lint: allow({rule})` needs a reason after it"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_path_line_rule_message() {
        let f = Finding {
            path: "src/a.rs".into(),
            line: 3,
            rule: "prefix-group",
            message: "m".into(),
        };
        assert_eq!(f.to_string(), "src/a.rs:3: [prefix-group] m");
    }

    #[test]
    fn an_allow_with_a_reason_excuses_its_line_and_the_item_below() {
        let src = "mod a;\n// lint: allow(prefix-group) mirrors Unity\n#[cfg(test)]\nmod b;\n";
        assert!(allowed(src, 4, "prefix-group"));
        assert!(allowed(src, 2, "prefix-group"));
        assert!(
            !allowed(src, 1, "prefix-group"),
            "a code line breaks the run"
        );
        assert!(!allowed(src, 4, "skip"), "an allow names one rule");
        assert!(!allowed(src, 9, "prefix-group"), "past the end");
    }

    #[test]
    fn an_allow_without_a_reason_excuses_nothing_and_is_reported() {
        let src = "// lint: allow(prefix-group)\nmod b;\n";
        assert!(!allowed(src, 2, "prefix-group"));
        let found = unexplained("x.rs", src);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 1);
        assert_eq!(found[0].rule, ALLOW);
        assert!(unexplained("x.rs", "// lint: allow(skip) flaky driver\n").is_empty());
    }

    #[test]
    fn the_marker_counts_only_inside_a_comment() {
        assert!(parse_allow("let s = \"lint: allow(x) y\";").is_none());
    }
}
