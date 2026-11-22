//! What the page is checked against that serde cannot list: the words a string field
//! is parsed for (a chord name's quality, a key's mode), and which types are documents.

use std::path::{Path, PathBuf};

/// Document types with no keys of their own: each only chooses between other types.
pub(super) const KEYLESS: &[&str] = &[
    "PatchRef",
    "PatternEntry",
    "Pitch",
    "ArrangementEntry",
    "Voicing",
    "Degree",
    "EntryBody",
];

/// Each chord quality, written on the page as a chord on C (`C`, `Cm7`, `C7#9`).
pub(super) fn qualities() -> Vec<String> {
    let text = source("song/chord/name.rs");
    let table = between(&text, "const QUALITIES", "];");
    quoted_before(table, ", &[")
        .map(|quality| format!("C{quality}"))
        .collect()
}

/// Each word a key's mode is spelled with (`major`, `dorian`).
pub(super) fn modes() -> Vec<String> {
    let text = source("song/key/mod.rs");
    let table = between(&text, "const SPELLINGS", "];");
    quoted_before(table, ", Mode::")
        .map(str::to_owned)
        .collect()
}

/// Every type under `src/patch/` and `src/song/` with a derived or hand-written `Deserialize`.
pub(super) fn deserialized() -> Vec<String> {
    let mut found = Vec::new();
    for dir in ["patch", "song"] {
        for file in rust_files(&src().join(dir)) {
            let text = std::fs::read_to_string(&file).expect("a source file reads");
            found.extend(declared(&text));
        }
    }
    found.sort();
    found.dedup();
    found
}

fn declared(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut derived = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with("#[derive(") {
            derived = line.contains("Deserialize");
        } else if let Some(rest) = line.strip_prefix("impl<'de> Deserialize<'de> for ") {
            found.push(ident(rest));
        } else if let Some(rest) = ["pub struct ", "pub enum "]
            .iter()
            .find_map(|head| line.strip_prefix(head))
        {
            if derived {
                found.push(ident(rest));
            }
            derived = false;
        }
    }
    found
}

fn ident(text: &str) -> String {
    let end = text.find(|c: char| !(c.is_alphanumeric() || c == '_'));
    text[..end.unwrap_or(text.len())].to_owned()
}

fn quoted_before<'a>(table: &'a str, after: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    table
        .split(after)
        .filter_map(|chunk| chunk.rsplit_once("(\""))
        .map(|(_, word)| {
            word.strip_suffix('"')
                .expect("a quoted word right before the separator")
        })
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("`{start}` is gone"));
    let rest = &text[from..];
    &rest[..rest.find(end).expect("the table ends")]
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn source(path: &str) -> String {
    std::fs::read_to_string(src().join(path)).unwrap_or_else(|_| panic!("src/{path} reads"))
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a source directory reads") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            found.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found
}

#[test]
fn a_derive_marks_the_type_it_sits_on() {
    let text = "#[derive(Clone, Deserialize)]\n#[serde(x)]\npub struct Kept {\n}\n\
                #[derive(Clone)]\npub enum Plain {}\nimpl<'de> Deserialize<'de> for Hand {";
    assert_eq!(declared(text), ["Kept", "Hand"]);
}
