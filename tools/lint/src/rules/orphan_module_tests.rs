use super::{check, src_paths, RULE};
use crate::repo::scaffold::Repo;
use crate::rules::scan::Scan;
use crate::rules::Finding;

const PACKAGE: &str = "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

/// A one-package repo whose `src/lib.rs` is `lib`.
fn repo(name: &str, lib: &str) -> Repo {
    let repo = Repo::new(&format!("orphan-module-{name}"), false);
    repo.file("Cargo.toml", PACKAGE);
    repo.file("src/lib.rs", lib);
    repo
}

fn hits(repo: &Repo) -> Vec<Finding> {
    check(&repo.0, &Scan::at(&repo.0))
}

fn found(repo: &Repo) -> Vec<String> {
    hits(repo)
        .into_iter()
        .inspect(|f| assert_eq!(f.rule, RULE))
        .map(|f| f.path)
        .collect()
}

#[test]
fn flags_a_file_beside_a_mod_rs_that_never_declares_it() {
    let dir = repo("beside", "mod a;\n");
    dir.file(
        "src/a/mod.rs",
        "mod b;\n// mod c;\nconst S: &str = \"mod c;\";\n",
    );
    dir.file("src/a/b.rs", "");
    dir.file("src/a/c.rs", "#[test]\nfn lost() {}\n");
    assert_eq!(found(&dir), ["src/a/c.rs"]);
}

#[test]
fn flags_a_folder_whose_parent_never_declares_it() {
    let dir = repo("folder", "mod a;\n");
    dir.file("src/a.rs", "mod b;\n");
    dir.file("src/a/b.rs", "");
    dir.file("src/gone/mod.rs", "mod inner;\n");
    dir.file("src/gone/inner.rs", "");
    let hits = hits(&dir);
    let paths: Vec<_> = hits.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/gone/inner.rs", "src/gone/mod.rs"]);
    assert!(hits[1].message.contains("`mod gone;`"), "{}", hits[1]);
}

#[test]
fn passes_path_cfg_test_inline_and_include() {
    let lib = "#[cfg(test)]\n#[path = \"x_tests.rs\"]\nmod tests;\n\
               #[cfg(test)]\nmod y_tests;\n\
               mod a {\n    mod b;\n    pub mod c {\n        mod d;\n    }\n}\n\
               mod after;\n\
               const A: &str = include_str!(\"after.rs\");\n\
               const T: &str = include_str!(\"../tests/data/fixture.rs\");\n";
    let dir = repo("pass", lib);
    for f in [
        "src/x_tests.rs",
        "src/y_tests.rs",
        "src/a/b.rs",
        "src/a/c/d.rs",
        "src/after/deep.rs",
        "tests/data/fixture.rs",
    ] {
        dir.file(f, "");
    }
    // Included first or not, a file that is also a module is still parsed.
    dir.file("src/after.rs", "mod deep;\n");
    assert_eq!(found(&dir), Vec::<String>::new());
}

#[test]
fn passes_cargo_targets_and_a_path_file_owning_its_folder() {
    let manifest = format!("{PACKAGE}[[bin]]\nname = \"odd\"\npath = \"tools/odd.rs\"\n");
    let dir = repo("targets", "");
    dir.file("Cargo.toml", &manifest);
    dir.file(
        "src/bin/one.rs",
        "#[path = \"../shared/p.rs\"]\nmod p;\nfn main() {}\n",
    );
    dir.file("src/shared/p.rs", "mod q;\n");
    dir.file("src/shared/q.rs", "");
    dir.file("src/bin/two/main.rs", "mod helper;\nfn main() {}\n");
    dir.file("src/bin/two/helper.rs", "");
    dir.file("tests/it.rs", "mod common;\n");
    dir.file("tests/common/mod.rs", "");
    dir.file("tools/odd.rs", "fn main() {}\n");
    assert_eq!(found(&dir), Vec::<String>::new());
}

#[test]
fn reports_a_workspace_cargo_cannot_read() {
    let dir = repo("broken", "");
    // An allow in the manifest excuses nothing: the manifest is what to fix.
    dir.file("Cargo.toml", "# lint: allow(orphan-module) try\n[package\n");
    assert_eq!(found(&dir)[0], "Cargo.toml");
}

#[test]
fn reads_src_paths_out_of_metadata() {
    let json = r#"{"targets":[{"src_path":"/r/a \"b\".rs"},{"src_path":"/r/c.rs"}]}"#;
    assert_eq!(src_paths(json), ["/r/a \"b\".rs", "/r/c.rs"]);
}

#[test]
fn literals_and_comments_keep_the_count_of_braces_and_brackets() {
    // Each `mod xN;` resolves only if what comes before it left the depth alone.
    let lib = "mod outer {\n    const S: &str = \"one\n    two\"; }\nmod x1;\n\
               mod m { fn f(c: char) -> bool { match c { '\"' => { true } _ => b'\"' == 0 } } }\n\
               mod x2;\n\
               #[doc = \"line one\nline two\"]\nmod x3;\n\
               mod x4; /* why */ mod x5;\n\
               const R: &str = r#\"mod nope;\n include_str!(\"nope.rs\") }\"#;\nmod x6;\n";
    let dir = repo("literals", lib);
    for f in ["x1", "x2", "x3", "x4", "x5", "x6", "nope"] {
        dir.file(&format!("src/{f}.rs"), "");
    }
    assert_eq!(found(&dir), ["src/nope.rs"]);
}

#[test]
fn attributes_count_only_path_and_their_includes() {
    let lib = "#![doc = include_str!(\"ex.rs\")]\n\
               #[cfg_attr(test, path = \"t.rs\")]\nmod a;\n\
               #[doc(alias = \"x\")]\n#[deprecated(note = \"x\")]\n#[foo(path = \"wrong.rs\")]\nmod w;\n";
    let dir = repo("attributes", lib);
    for f in ["ex", "t", "a", "w", "wrong"] {
        dir.file(&format!("src/{f}.rs"), "");
    }
    assert_eq!(found(&dir), ["src/wrong.rs"]);
}

#[test]
fn a_file_loaded_two_ways_resolves_both() {
    let dir = repo(
        "two-ways",
        "#[path = \"shared/p.rs\"]\nmod p;\nmod shared;\n",
    );
    dir.file("src/shared/mod.rs", "mod p;\n");
    dir.file("src/shared/p.rs", "mod q;\n");
    dir.file("src/shared/q.rs", "");
    dir.file("src/shared/p/q.rs", "");
    assert_eq!(found(&dir), Vec::<String>::new());
}

#[test]
fn an_allow_excuses_a_file_and_a_file_above_every_crate_is_told_so() {
    let dir = repo("allow", "");
    dir.file(
        "tools/script.rs",
        "// lint: allow(orphan-module) run with `rustc`\n",
    );
    dir.file("tools/stray.rs", "");
    let hits = hits(&dir);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].path, "tools/stray.rs");
    assert!(
        hits[0].message.contains("no crate root sits above it"),
        "{}",
        hits[0]
    );
    assert!(
        hits[0].message.contains("allow(orphan-module)"),
        "{}",
        hits[0]
    );
}

#[test]
fn a_nested_workspace_header_may_carry_a_comment() {
    let dir = repo("nested", "");
    let manifest = format!(
        "{}[workspace] # standalone\n",
        PACKAGE.replace("\"t\"", "\"n\"")
    );
    dir.file("tools/n/Cargo.toml", &manifest);
    dir.file("tools/n/src/lib.rs", "mod m;\n");
    dir.file("tools/n/src/m.rs", "");
    assert_eq!(found(&dir), Vec::<String>::new());
}
