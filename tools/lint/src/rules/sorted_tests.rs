use super::{check, markdown, RULE};

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
fn finds_markdown_under_docs_and_at_the_root() {
    let root = std::env::temp_dir().join(format!("lint_sorted_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("docs/api")).expect("temp dir");
    for f in ["README.md", "docs/api/A.md", "docs/a.txt", "src.md"] {
        std::fs::write(root.join(f), "").expect("write");
    }
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(root.join("src/b.md"), "").expect("write");
    let found = markdown(&root);
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(found, ["README.md", "docs/api/A.md", "src.md"]);
}
