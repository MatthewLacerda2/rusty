//! API-doc layout gate (#569): `docs/api/` holds one file per Lua namespace, named
//! after it, and `index.md` links every one. The drift gates read the directory as
//! one assembled document (`build.rs`), so a misfiled section would still pass them;
//! this keeps the split itself honest — `docs/api/Physics.md` documents `Physics`,
//! and no namespace file is missing from the index an agent starts at.

use std::fs;
use std::path::Path;

#[test]
fn every_namespace_file_is_named_after_its_namespace_and_indexed() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/api");
    let index = fs::read_to_string(dir.join("index.md")).expect("docs/api/index.md");
    let mut problems = Vec::new();
    for entry in fs::read_dir(&dir).expect("docs/api/") {
        let name = entry
            .expect("dir entry")
            .file_name()
            .into_string()
            .expect("utf-8");
        let Some(stem) = name.strip_suffix(".md").filter(|s| *s != "index") else {
            continue;
        };
        let text = fs::read_to_string(dir.join(&name)).expect("namespace file");
        if !text.starts_with(&format!("## `{stem}`")) {
            problems.push(format!("{name} must start with the heading ## `{stem}`"));
        }
        if !index.contains(&format!("]({name})")) {
            problems.push(format!("index.md does not link {name}"));
        }
    }
    assert!(
        problems.is_empty(),
        "docs/api/ layout:\n  {}",
        problems.join("\n  ")
    );
}
