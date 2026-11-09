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
    // naming serde with no codec call and no type derived beside it is no document
    let named = "fn f(v: &serde_json::Value) {\n    std::fs::read(p);\n}\n";
    assert!(found(&[("src/a/io.rs", named)]).is_empty());
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

/// The cases `.github/scripts`'s `covering` answers too, from one table: a
/// mirror that drifts fails here or in `make scripts`.
#[test]
fn coverage_reads_the_shared_table_as_review_level_does() {
    let rows = include_str!("stored_format_cover.tsv")
        .lines()
        .filter(|l| !l.starts_with("# ") && !l.is_empty());
    for row in rows {
        let cols: Vec<&str> = row.split('\t').collect();
        let marked: Vec<&str> = cols[1].split(',').collect();
        assert_eq!(covered(cols[0], &marked), cols[2] == "yes", "{row}");
    }
}

#[test]
fn flags_a_codec_call_whose_type_is_derived_elsewhere() {
    // `.physmat`'s shape: the type lives in `components/`, the file in `scene/`
    let load = "fn load(p: &Path) -> M {\n    let t = std::fs::read_to_string(p).unwrap();\n    serde_json::from_str(&t).unwrap()\n}\n";
    let files = [("src/components/m.rs", TYPE), ("src/scene/m/file.rs", load)];
    assert_eq!(found(&files), [("src/scene/m/file.rs".into(), 2)]);
    // and through `written::write`, the one engine asset write
    let save = "fn save(m: &M) {\n    let t = serde_json::to_vec(m).unwrap();\n    written::write(p, &t).unwrap();\n}\n";
    assert_eq!(found(&[("src/x/f.rs", save)]), [("src/x/f.rs".into(), 3)]);
}

#[test]
fn reports_every_io_line_so_one_allow_excuses_only_its_own() {
    let src = format!("{TYPE}fn f() {{\n    // lint: allow(stored-format) reads a script\n    std::fs::read(p);\n}}\n{SAVE}");
    let lines: Vec<usize> = found(&[("src/a.rs", &src)]).iter().map(|f| f.1).collect();
    assert_eq!(lines, [5, 9]);
    assert!(crate::rules::allowed(&src, 5, RULE));
    assert!(!crate::rules::allowed(&src, 9, RULE));
}

#[test]
fn reads_a_wrapped_derive_and_an_indented_marker() {
    let wrapped = "#[derive(\n    Debug,\n    Serialize,\n)]\npub struct Doc;\nfn f() { serde::x(); std::fs::read(p); }\n";
    assert_eq!(
        found(&[("src/a/doc.rs", wrapped)]),
        [("src/a/doc.rs".into(), 6)]
    );
    let indented = format!("  //! stored-format: `d.json`\n{TYPE}{SAVE}");
    assert!(found(&[("src/a/doc.rs", &indented)]).is_empty());
}
