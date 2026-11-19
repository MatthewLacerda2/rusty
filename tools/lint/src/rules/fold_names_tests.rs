use super::{check, run, RULE};
use crate::repo::scaffold::Repo;
use crate::rules::scan::Scan;

fn members() -> Vec<(String, String)> {
    vec![("rusty_font".to_string(), "font".to_string())]
}

#[test]
fn flags_a_spelling_the_fold_cannot_rewrite() {
    let src = "use rusty_font::Glyph;\npub use rusty_font as font;\nuse rusty;\n";
    let found: Vec<_> = check("crates/ui/src/a.rs", src, "rusty", &members())
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(2, RULE), (3, RULE)]);
}

#[test]
fn passes_every_spelling_the_fold_rewrites() {
    let src = "use rusty_font as font;\nuse rusty_font;\nextern crate rusty_font;\n\
               use {rusty_font as f, x};\nm!(rusty_font);\nuse rusty as r;\n\
               pub use rusty_font::{Glyph};\nlet s = \"rusty_font\";\n\
               let rusty = 1;\nlet v = vec![rusty, x];\nlet k = Foo { rusty, .. };\n";
    assert!(check("crates/ui/src/a.rs", src, "rusty", &members()).is_empty());
}

#[test]
fn flags_a_workspace_it_cannot_read() {
    let repo = Repo::new("fold-names-unreadable", true);
    repo.file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/gone\"]\n\n[package]\nname = \"rusty\"\n",
    );
    repo.file("src/lib.rs", "");
    repo.git(&["add", "-A"]);
    let found: Vec<_> = run(&Scan::at(&repo.0))
        .into_iter()
        .map(|f| (f.path, f.rule))
        .collect();
    assert_eq!(found, [("Cargo.toml".to_string(), RULE)]);
}

/// Through the scan: a member's, a bin's and a test's file are read; the root
/// lib, whose re-exports the fold mounts, and a file outside the fold are not.
#[test]
fn reads_only_the_files_the_fold_rewrites() {
    let repo = Repo::new("fold-names-scope", true);
    let bad = "pub use rusty_font as font;\n";
    repo.file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\n\n[package]\nname = \"rusty\"\n\n\
         [dependencies]\nrusty-font = { path = \"crates/font\" }\n\n\
         [[bin]]\nname = \"b\"\npath = \"src/bin/b.rs\"\n",
    );
    repo.file(
        "crates/font/Cargo.toml",
        "[package]\nname = \"rusty-font\"\n",
    );
    repo.file("crates/font/src/lib.rs", "");
    repo.file("crates/ui/Cargo.toml", "[package]\nname = \"rusty-ui\"\n");
    repo.file("crates/ui/src/lib.rs", bad);
    repo.file("src/lib.rs", bad);
    repo.file("src/bin/b.rs", bad);
    repo.file("src/bin/b/m.rs", bad);
    repo.file("tests/main.rs", bad);
    repo.file("tools/x/src/a.rs", bad);
    repo.git(&["add", "-A"]);
    let mut got: Vec<String> = run(&Scan::at(&repo.0))
        .into_iter()
        .map(|f| f.path)
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            "crates/ui/src/lib.rs",
            "src/bin/b.rs",
            "src/bin/b/m.rs",
            "tests/main.rs"
        ]
    );
}
