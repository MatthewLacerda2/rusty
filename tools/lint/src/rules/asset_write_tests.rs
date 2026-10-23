use super::{check, RULE};

fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &[]).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_wav_written_with_no_note() {
    let src = "fn bake(p: &Path) {\n    std::fs::write(p, wav)?;\n}\n";
    assert_eq!(lines("src/a.rs", src), [2]);
    assert!(check("src/a.rs", src, &[]).iter().all(|f| f.rule == RULE));
    assert_eq!(lines("src/a.rs", "let f = File::create(p)?;\n"), [1]);
    assert_eq!(lines("src/a.rs", "img.save(&p)?;\n"), [1]);
}

#[test]
fn passes_the_helper_a_note_and_an_empty_save() {
    assert!(lines("src/a.rs", "written::write(&p, &bytes)?;\n").is_empty());
    let noted = "fn f() {\n    image.save(&p)?;\n    note_written(&p);\n}\n";
    assert!(lines("src/a.rs", noted).is_empty());
    assert!(lines("src/a.rs", "self.save()?;\n").is_empty());
}

#[test]
fn a_note_in_the_next_function_does_not_count() {
    let src = "fn a() {\n    fs::write(p, b)?;\n}\nfn b() {\n    note_written(p);\n}\n";
    assert_eq!(lines("src/a.rs", src), [2]);
}

#[test]
fn skips_tests_comments_the_home_and_non_engine_code() {
    let w = "std::fs::write(p, b)?;\n";
    assert!(check("src/a.rs", w, &[true]).is_empty());
    assert!(lines("src/a.rs", "// std::fs::write(p, b)\n").is_empty());
    assert!(lines("src/core/fs/mod.rs", w).is_empty());
    assert!(lines("tests/a.rs", w).is_empty());
    assert!(lines("tools/lint/src/a.rs", w).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src =
        "// lint: allow(asset-write) not read by a path-keyed cache\nstd::fs::write(p, b)?;\n";
    let found = check("src/a.rs", src, &[]);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
