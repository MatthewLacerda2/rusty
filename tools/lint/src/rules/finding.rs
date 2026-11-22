use std::fmt;

/// The rule an allow that gives no reason is reported under.
pub const ALLOW: &str = "allow";
const MARKER: &str = "lint: allow(";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Repo-relative, `/`-separated.
    pub path: String,
    /// 1-based: the line an allow for this finding anchors to.
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

/// `// lint: allow(<rule>) <why>`, Lua's `-- lint: allow(<rule>) <why>`, or Markdown's
/// `<!-- lint: allow(<rule>) <why> -->`, whose reason ends at the `-->`. Each file
/// type reads only its own form, so code quoting another form in a string is no allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Code,
    Lua,
    Markdown,
    /// Python, a table, a manifest: `# lint: allow(<rule>) <why>`.
    Hash,
}

impl Form {
    pub fn of(path: &str) -> Form {
        if path.ends_with(".md") {
            Form::Markdown
        } else if path.ends_with(".lua") {
            Form::Lua
        } else if [".py", ".txt", ".toml"].iter().any(|e| path.ends_with(e)) {
            Form::Hash
        } else {
            Form::Code
        }
    }

    fn opener(self) -> &'static str {
        match self {
            Form::Code => "//",
            Form::Lua => "--",
            Form::Markdown => "<!--",
            Form::Hash => "#",
        }
    }

    fn continues(self, line: &str) -> bool {
        line.starts_with(self.opener()) || (self == Form::Code && line.starts_with("#["))
    }
}

pub(super) fn parse_allow(line: &str, form: Form) -> Option<(&str, &str)> {
    // comment-cut: this reads the comment itself, never the code before it
    let comment = &line[line.find(form.opener())?..];
    let rest = &comment[comment.find(MARKER)? + MARKER.len()..];
    let close = rest.find(')')?;
    let why = &rest[close + 1..];
    let why = match form {
        Form::Markdown => why.split_once("-->").map_or(why, |(w, _)| w),
        Form::Code | Form::Lua | Form::Hash => why,
    };
    Some((rest[..close].trim(), why.trim()))
}

pub fn allowed(source: &str, line: usize, rule: &str) -> bool {
    allowed_as(Form::Code, source, line, rule)
}

pub fn allowed_in(path: &str, source: &str, line: usize, rule: &str) -> bool {
    allowed_as(Form::of(path), source, line, rule)
}

/// For a rule that belongs to no line (a length or word cap): an allow on any line.
pub fn allowed_anywhere(path: &str, source: &str, rule: &str) -> bool {
    source.lines().any(|line| allowed_in(path, line, 1, rule))
}

fn allowed_as(form: Form, source: &str, line: usize, rule: &str) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let Some(at) = line.checked_sub(1).filter(|&i| i < lines.len()) else {
        return false;
    };
    let excuses =
        |l: &str| parse_allow(l, form).is_some_and(|(r, why)| r == rule && !why.is_empty());
    if excuses(lines[at]) {
        return true;
    }
    lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim_start())
        .take_while(|l| form.continues(l))
        .any(excuses)
}

pub fn unexplained(path: &str, source: &str) -> Vec<Finding> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let (rule, why) = parse_allow(l, Form::of(path))?;
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
    fn the_markdown_form_reads_its_reason_up_to_the_comment_close() {
        let md = "<!-- lint: allow(history) a quoted message -->";
        assert_eq!(
            parse_allow(md, Form::Markdown),
            Some(("history", "a quoted message"))
        );
        let bare = format!("<!-- lint: {}allow(history) -->", "");
        assert_eq!(parse_allow(&bare, Form::Markdown), Some(("history", "")));
        let url = "see https://x.y <!-- lint: allow(history) why --> tail";
        assert_eq!(parse_allow(url, Form::Markdown), Some(("history", "why")));
        let src = format!("{md}\nsee #12\n");
        assert!(allowed_in("CLAUDE.md", &src, 2, "history"));
        assert!(!allowed_in(
            "CLAUDE.md",
            &format!("{bare}\nsee #12\n"),
            2,
            "history"
        ));
        assert_eq!(unexplained("a/CLAUDE.md", &bare).len(), 1);
    }

    #[test]
    fn each_file_reads_only_its_own_form() {
        let html = format!("let s = \"<!-- lint: {}allow(x) y -->\";", "");
        assert!(parse_allow(&html, Form::Code).is_none(), "a string in code");
        assert!(!allowed_in("a.rs", &html, 1, "x"));
        let slashes = format!("// lint: {}allow(x) y", "");
        assert!(parse_allow(&slashes, Form::Markdown).is_none());
        assert!(!allowed_in("a.md", &slashes, 1, "x"));
        assert_eq!(Form::of("a/SKILL.md"), Form::Markdown);
        assert_eq!(Form::of("a.rs"), Form::Code);
    }

    #[test]
    fn a_code_allow_after_a_string_quoting_the_marker_still_counts() {
        let line = format!(
            "let s = \"lint: {0}allow(x) q\"; // lint: {0}allow(y) why",
            ""
        );
        assert_eq!(parse_allow(&line, Form::Code), Some(("y", "why")));
        assert!(allowed_in("a.rs", &line, 1, "y"));
    }

    #[test]
    fn the_marker_counts_only_inside_a_comment() {
        assert!(parse_allow("let s = \"lint: allow(x) y\";", Form::Code).is_none());
    }
}
