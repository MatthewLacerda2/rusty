//! What `docs/api/Sound.md` documents: the names its tables carry.

use std::collections::BTreeSet;

pub(super) struct Page {
    /// Every name in backticks in a table row's first cell: the keys given a row.
    pub(super) rows: BTreeSet<String>,
    /// Every name in backticks anywhere in a table row: the words a key takes.
    pub(super) words: BTreeSet<String>,
}

pub(super) fn read() -> Page {
    let path = rusty_core::project::checkout().join("docs/api/Sound.md");
    let text = std::fs::read_to_string(&path).expect("docs/api/Sound.md reads");
    parse(&text)
}

pub(super) fn parse(text: &str) -> Page {
    let mut page = Page {
        rows: BTreeSet::new(),
        words: BTreeSet::new(),
    };
    for line in text.lines().filter(|line| line.starts_with('|')) {
        let first = line.split('|').nth(1).unwrap_or("");
        page.rows.extend(names(first));
        page.words.extend(names(line));
    }
    page
}

/// The backticked spans of `text`, whole and cut into their names: `tracks[].gain`
/// gives itself, `tracks` and `gain`; `"saw"` gives `saw`.
fn names(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for span in text.split('`').skip(1).step_by(2) {
        let span = span.trim_matches('"');
        found.push(span.to_owned());
        let piece = |c: char| !(c.is_ascii_alphanumeric() || c == '_');
        found.extend(
            span.split(piece)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        );
    }
    found
}

#[test]
fn a_row_names_its_keys_and_its_cells_name_words() {
    let page = parse("| `tracks[].gain` / `pan` | `1.0` | `\"saw\"` or `C#m7` |\nprose `fade`\n");
    for row in ["tracks[].gain", "tracks", "gain", "pan"] {
        assert!(page.rows.contains(row), "{row}");
    }
    assert!(!page.rows.contains("saw"), "a later cell is no row");
    assert!(page.words.contains("saw") && page.words.contains("C#m7"));
    assert!(
        !page.words.contains("fade"),
        "prose outside a table counts for nothing"
    );
}
