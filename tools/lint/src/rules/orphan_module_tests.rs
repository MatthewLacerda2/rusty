use super::{check, src_paths, RULE};
use crate::repo::scaffold::Repo;
use crate::rules::scan::Scan;

const PACKAGE: &str = "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

/// A one-package repo whose `src/lib.rs` is `lib`.
fn repo(name: &str, lib: &str) -> Repo {
    let repo = Repo::new(&format!("orphan-module-{name}"), false);
    repo.file("Cargo.toml", PACKAGE);
    repo.file("src/lib.rs", lib);
    repo
}

fn found(repo: &Repo) -> Vec<String> {
    let files = Scan::at(&repo.0).files;
    check(&repo.0, &files)
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
    let files = Scan::at(&dir.0).files;
    let hits = check(&dir.0, &files);
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
    dir.file("Cargo.toml", "[package\n");
    assert_eq!(found(&dir)[0], "Cargo.toml");
}

#[test]
fn reads_src_paths_out_of_metadata() {
    let json = r#"{"targets":[{"src_path":"/r/a \"b\".rs"},{"src_path":"/r/c.rs"}]}"#;
    assert_eq!(src_paths(json), ["/r/a \"b\".rs", "/r/c.rs"]);
}
