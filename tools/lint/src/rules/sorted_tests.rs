use super::{check, markdown, markdown_findings, RULE};
use crate::repo::scaffold::Repo;
use std::fs;
use std::path::PathBuf;

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
        [
            PathBuf::from("README.md"),
            PathBuf::from("docs/api/A.md"),
            PathBuf::from("src.md")
        ]
    );
}

const BAD: &str = "<!-- sorted: an index -->\n- b\n- a\n";

#[test]
fn fails_on_a_listed_file_it_cannot_read() {
    let repo = Repo::new("sorted-gone", true);
    let kept = repo.file("docs/ok.md", BAD);
    let gone = repo.file("docs/gone.md", "");
    repo.git(&["add", "docs"]);
    fs::remove_file(gone).expect("remove");
    assert!(kept.exists());
    let found = markdown_findings(&repo.0);
    let at: Vec<_> = found.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(at, ["docs/gone.md", "docs/ok.md"], "{found:?}");
    assert!(found[0].message.contains("could not read"), "{found:?}");
}

// Linux only: APFS refuses a file name that isn't UTF-8.
#[cfg(target_os = "linux")]
#[test]
fn checks_a_markdown_file_whose_name_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Repo::new("sorted-bytes", true);
    let name = std::ffi::OsStr::from_bytes(b"docs/\xff.md");
    fs::create_dir_all(repo.0.join("docs")).expect("dirs");
    fs::write(repo.0.join(name), BAD).expect("write");
    repo.git(&["add", "-A"]);
    let found = markdown_findings(&repo.0);
    assert_eq!(lines(&found), [(3, RULE)], "{found:?}");
}

#[test]
fn reads_a_hyphenated_keyword_word_as_a_name() {
    // `self-path` sorts under `s`, not as `path` after the keyword `self` (#1654).
    let ok = "<!-- sorted: rules -->\n- **`sanitize`**\n- **`self-path`**\n- **`skip`**\n";
    assert!(check("docs/x.md", ok).is_empty());
    let bad = "<!-- sorted: rules -->\n- **`self-path`**\n- **`sanitize`**\n";
    assert_eq!(lines(&check("docs/x.md", bad)), [(3, RULE)]);
}
