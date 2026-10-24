use super::{check, markdown, RULE};
use crate::repo::scaffold::Repo;

fn lines(found: &[crate::rules::Finding]) -> Vec<(usize, &str)> {
    found.iter().map(|f| (f.line, f.rule)).collect()
}

#[test]
fn flags_an_entry_out_of_order() {
    let src = "// sorted: names only\nmod b;\nmod a;\nmod c;\n";
    assert_eq!(lines(&check("src/x.rs", src)), [(3, RULE)]);
    let md = "<!-- sorted: an index -->\n- [`Sfx`](Sfx.md)\n- [`Audio`](Audio.md)\n";
    assert_eq!(lines(&check("docs/x.md", md)), [(3, RULE)]);
}

#[test]
fn flags_a_marker_with_no_reason() {
    assert_eq!(
        lines(&check("src/x.rs", "// sorted:\nmod a;\n")),
        [(1, RULE)]
    );
}

#[test]
fn passes_a_sorted_list_whatever_its_visibility_and_notes() {
    let src = "// sorted: names only\npub mod a;\n#[cfg(test)]\npub(crate) mod b;\n\
               // c is a note\nmod c;\nmod c_d;\n";
    assert!(check("src/x.rs", src).is_empty());
    let calls = "    // sorted: registrars\n    Audio::register(\n        x,\n    )?;\n    \
                 bake::register(x)?;\n    bake::register_more(x)\n}\nfn z() {}\n";
    assert!(check("src/x.rs", calls).is_empty());
}

#[test]
fn ends_the_list_at_a_blank_line_and_skips_a_table_header() {
    let src = "// sorted: names only\nmod a;\nmod b;\n\nmod a;\n";
    assert!(check("src/x.rs", src).is_empty());
    let md = "<!-- sorted: rows -->\n| Zed | Note |\n|---|---|\n| a | x |\n| b | y |\n";
    assert!(check("docs/x.md", md).is_empty());
}

#[test]
fn passes_an_unmarked_list() {
    assert!(check("src/x.rs", "mod b;\nmod a;\n").is_empty());
}

#[test]
fn flags_a_marker_that_governs_no_entries() {
    let blank = "// sorted: names only\n\nmod b;\nmod a;\n";
    assert_eq!(lines(&check("src/x.rs", blank)), [(1, RULE)]);
    let wrapped = "<!-- sorted: an index,\nwrapped -->\n- b\n- a\n";
    assert_eq!(lines(&check("docs/x.md", wrapped)), [(1, RULE)]);
    let deeper = "// sorted: names only\n    mod b;\nmod a;\n";
    assert_eq!(lines(&check("src/x.rs", deeper)), [(1, RULE)]);
}

#[test]
fn skips_a_table_header_only_right_under_the_marker() {
    let md = "<!-- sorted: rows -->\n- b\n- a\n| Col | Note |\n|---|---|\n| d | x |\n";
    assert_eq!(lines(&check("docs/x.md", md)), [(3, RULE)]);
}

#[test]
fn finds_tracked_markdown_under_docs_and_at_the_root() {
    let repo = Repo::new("sorted-md", true);
    let files = [
        "README.md",
        "docs/api/A.md",
        "docs/a.txt",
        "src.md",
        "src/b.md",
    ];
    for f in files.iter().chain(&["notes.md"]) {
        repo.file(f, "");
    }
    repo.git(&[&["add", "--"][..], &files].concat());
    assert_eq!(
        markdown(&repo.0).expect("git"),
        ["README.md", "docs/api/A.md", "src.md"]
    );
}
