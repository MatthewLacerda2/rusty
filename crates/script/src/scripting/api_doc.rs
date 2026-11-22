//! It reads the checkout at run time rather than embedding the text at build time,
//! because an embedded doc is part of the crate's fingerprint: every doc edit would
//! rebuild the whole crate once per feature set. Its callers are tests and
//! `dev` tools, which always run from the engine checkout.

use std::fs;
use std::path::{Path, PathBuf};

pub fn api_doc_dir() -> PathBuf {
    rusty_core::project::checkout().join("docs").join("api")
}

pub fn api_reference() -> Result<String, String> {
    assemble(&api_doc_dir())
}

pub fn assemble(dir: &Path) -> Result<String, String> {
    let read = |name: &str| {
        fs::read_to_string(dir.join(name))
            .map_err(|e| format!("Failed to read {}: {e}", dir.join(name).display()))
    };
    let index = read("index.md")?;
    let mut order: Vec<String> = index
        .split("](")
        .skip(1)
        .filter_map(|rest| rest.split_once(')'))
        .map(|(target, _)| target.to_string())
        .filter(|t| t.ends_with(".md") && !t.contains('/') && dir.join(t).is_file())
        .collect();
    let mut rest: Vec<String> = fs::read_dir(dir)
        .map_err(|e| format!("Failed to list {}: {e}", dir.display()))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md") && n != "index.md" && !order.contains(n))
        .collect();
    rest.sort();
    order.dedup();
    order.extend(rest);

    let mut doc = index;
    for name in &order {
        doc.push_str("\n---\n\n");
        doc.push_str(&read(name)?);
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_keeps_linked_then_unlinked_namespace_files_once() {
        let dir = rusty_test_support::temp::dir().join("api_doc");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        let index = "# Index\n[z](z.md) [a](a.md) [b](sub/b.md) [n](notes.txt) [m](missing.md)\n";
        for (name, text) in [
            ("index.md", index),
            ("z.md", "Z\n"),
            ("a.md", "A\n"),
            ("extra.md", "E\n"),
            ("notes.txt", "N\n"),
            ("sub/b.md", "B\n"),
        ] {
            fs::write(dir.join(name), text).unwrap();
        }
        let doc = assemble(&dir);
        let _ = fs::remove_dir_all(&dir);
        let part = |text: &str| format!("\n---\n\n{text}");
        let want = [index.to_string(), part("Z\n"), part("A\n"), part("E\n")].concat();
        assert_eq!(doc, Ok(want));
    }
}
