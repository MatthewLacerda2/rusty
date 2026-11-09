use super::{check, covered, Source, RULE};

const TYPE: &str = "#[derive(Serialize, Deserialize)]\npub struct Doc { a: u32 }\n";
const SAVE: &str = "pub fn save(d: &Doc) {\n    let t = serde_json::to_string(d).unwrap();\n    std::fs::write(\"d.json\", t).unwrap();\n}\n";

fn found(files: &[(&str, &str)]) -> Vec<(String, usize)> {
    let sources: Vec<Source> = files
        .iter()
        .map(|(path, text)| Source {
            path,
            text,
            test: &[],
        })
        .collect();
    check(&sources)
        .into_iter()
        .map(|f| (f.path, f.line))
        .collect()
}

#[test]
fn flags_a_serde_type_written_to_disk_without_a_marker() {
    let one = format!("{TYPE}{SAVE}");
    assert_eq!(
        found(&[("src/a/doc.rs", &one)]),
        [("src/a/doc.rs".into(), 5)]
    );
    // the type in one file, the write in its sibling: still one module
    let split = found(&[("src/a/doc.rs", TYPE), ("src/a/io.rs", SAVE)]);
    assert_eq!(split, [("src/a/io.rs".into(), 3)]);
    assert_eq!(
        check(&[Source {
            path: "src/a.rs",
            text: &one,
            test: &[]
        }])[0]
            .rule,
        RULE
    );
}

#[test]
fn passes_the_same_module_once_marked() {
    let marked = format!("//! the doc\n//! stored-format: `d.json`\n{TYPE}{SAVE}");
    assert!(found(&[("src/a/doc.rs", &marked)]).is_empty());
    let by_mod = [
        ("src/a/mod.rs", "//! stored-format: `d.json`\n"),
        ("src/a/b/io.rs", SAVE),
        ("src/a/b/doc.rs", TYPE),
    ];
    assert!(found(&by_mod).is_empty());
    let by_file = [
        ("src/a/b.rs", "//! stored-format: `d.json`\n"),
        ("src/a/b/io.rs", SAVE),
        ("src/a/b/doc.rs", TYPE),
    ];
    assert!(found(&by_file).is_empty());
}

#[test]
fn passes_serde_that_never_touches_a_file() {
    let snapshot =
        format!("{TYPE}pub fn send(d: &Doc) -> String {{ serde_json::to_string(d).unwrap() }}\n");
    assert!(found(&[("src/mcp/msg.rs", &snapshot)]).is_empty());
    // a file read with no serde type in the directory is not a document
    assert!(found(&[("src/a/io.rs", SAVE)]).is_empty());
    // `fs::read_dir` lists a folder, it reads no file
    let list = format!("{TYPE}fn f() {{ serde_json::x(); std::fs::read_dir(p); }}\n");
    assert!(found(&[("src/a/doc.rs", &list)]).is_empty());
    // test code and code outside `src/` are not the engine's formats
    let one = format!("{TYPE}{SAVE}");
    assert!(found(&[("tests/a.rs", &one), ("tools/x/a.rs", &one)]).is_empty());
    let test = [true; 6];
    assert!(check(&[Source {
        path: "src/a.rs",
        text: &one,
        test: &test
    }])
    .is_empty());
}

#[test]
fn an_empty_marker_is_a_finding() {
    assert_eq!(
        found(&[("src/a.rs", "//! x\n//! stored-format:\n")]),
        [("src/a.rs".into(), 2)]
    );
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = format!("{TYPE}fn f() {{\n    serde_json::x();\n    // lint: allow(stored-format) reads a script\n    std::fs::read(p);\n}}\n");
    let line = found(&[("src/a.rs", &src)])[0].1;
    assert!(crate::rules::allowed(&src, line, RULE));
}

#[test]
fn coverage_is_the_file_its_owners_and_their_ancestors() {
    assert!(covered("src/a/b/c.rs", &["src/a/mod.rs"]));
    assert!(covered("src/a/b/c.rs", &["src/a/b.rs"]));
    assert!(covered("src/a/b/c.rs", &["src/a/b/c.rs"]));
    assert!(!covered("src/a/b/c.rs", &["src/a/b/d.rs"]));
    assert!(!covered("src/a/bc/c.rs", &["src/a/b.rs"]));
}
