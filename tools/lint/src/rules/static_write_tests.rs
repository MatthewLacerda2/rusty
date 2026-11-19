use super::{check_file, decls, RULE};
use crate::code::bare_lines;

/// The lines `run` would flag in one file: its own statics are the names it knows.
fn check(path: &str, source: &str) -> Vec<usize> {
    let test = crate::rules::test_code::test_lines(path, source, false);
    let bare = bare_lines(source);
    let names = decls(&bare).process;
    check_file(path, source, &bare, &test, &|n| names.contains(n))
        .iter()
        .map(|f| f.line)
        .collect()
}

/// The lines of `body`, put in a test after [`DECL`], that the rule flags.
fn in_test(body: &str) -> Vec<usize> {
    let lines: Vec<usize> = check("crates/app/src/a_tests.rs", &format!("{DECL}{body}\n"))
        .iter()
        .map(|l| l - DECL.lines().count())
        .collect();
    lines
}

const DECL: &str = "static ON: AtomicBool = AtomicBool::new(false);\nstatic N: AtomicUsize = AtomicUsize::new(0);\nstatic SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());\n";

#[test]
fn flags_a_test_storing_to_a_crate_static() {
    let src = format!("{DECL}#[cfg(test)]\nmod tests {{\n    fn t() {{\n        ON.store(true, Relaxed);\n        super::N.fetch_add(1, Relaxed);\n        SEEN.lock().unwrap().push(3);\n        *SEEN.lock().unwrap() = Vec::new();\n    }}\n}}\n");
    assert_eq!(check("crates/app/src/a.rs", &src), [7, 8, 9, 10]);
    assert_eq!(
        check(
            "tests/a.rs",
            "static F: OnceLock<u8> = OnceLock::new();\nfn t() { F.set(1); F.get_or_init(|| 2); }\n"
        ),
        [2]
    );
}

#[test]
fn flags_a_write_through_any_guard_and_a_split_chain() {
    for write in [
        "SEEN.lock().unwrap_or_else(|e| e.into_inner()).insert(1);",
        "SEEN.lock().expect(\"held\").push_str(\"a\");",
        "SEEN.lock().unwrap().pop_front();",
        "SEEN.lock().unwrap().sort();",
        "SEEN.lock().unwrap().append(&mut v);",
        "SEEN.lock().unwrap().count += 1;",
        "SEEN.lock().unwrap().items.push(1);",
        "SEEN.lock().unwrap()[0] = 1;",
        "SEEN.lock().unwrap().replace(3);",
        "SEEN\n    .lock()\n    .unwrap_or_else(|e| e.into_inner())\n    .insert(k);",
    ] {
        assert_eq!(in_test(write), [1], "{write}");
    }
}

#[test]
fn passes_a_read_through_a_guard() {
    for read in [
        "assert_eq!(SEEN.lock().unwrap().iter().take(2).count(), 2);",
        "let s = SEEN.lock().unwrap().replace(\"a\", \"b\");",
        "let n = SEEN.lock().expect(\"held\").len();",
        "assert!(SEEN.lock().unwrap().count <= 2);",
        "assert!(SEEN.lock().unwrap().count == 2);",
        "let n = SEEN\n    .lock()\n    .unwrap()\n    .len();",
    ] {
        assert!(in_test(read).is_empty(), "{read}");
    }
}

#[test]
fn flags_a_test_hook_that_writes_one() {
    let src = "static ARMED: AtomicBool = AtomicBool::new(false);\n\n/// Count every walk from now on. A test hook.\n#[doc(hidden)]\npub fn arm() {\n    ARMED.store(true, Relaxed);\n}\n";
    assert_eq!(check("crates/ecs/src/walks.rs", src), [6]);
    let module = "//! Walk counts. A test hook: compiled in every build.\nstatic ARMED: AtomicBool = AtomicBool::new(false);\n\npub fn arm() {\n    ARMED.store(true, Relaxed);\n}\n";
    assert_eq!(check("crates/ecs/src/walks.rs", module), [5]);
}

#[test]
fn a_files_own_static_wins_over_a_neighbours_thread_local() {
    let root = std::env::temp_dir().join(format!("lint_static_write_{}", std::process::id()));
    let src = root.join("crates/app/src");
    std::fs::create_dir_all(&src).unwrap();
    let file = |path: &str, text: &str| std::fs::write(src.join(path), text).unwrap();
    file("a_tests.rs", "static ARMED: AtomicBool = AtomicBool::new(false);\n#[test]\nfn t() {\n    ARMED.store(true, Relaxed);\n}\n");
    file("b_tests.rs", "thread_local! {\n    static ARMED: Cell<bool> = const { Cell::new(false) };\n}\n#[test]\nfn t() {\n    ARMED.set(true);\n}\n");
    let found = super::run(&crate::rules::scan::Scan::at(&root));
    std::fs::remove_dir_all(&root).ok();
    let found: Vec<(&str, usize)> = found.iter().map(|f| (f.path.as_str(), f.line)).collect();
    assert_eq!(found, [("crates/app/src/a_tests.rs", 4)]);
}

#[test]
fn passes_thread_locals_reads_production_writes_and_other_roots() {
    let local = "thread_local! {\n    static ON: Cell<bool> = const { Cell::new(false) };\n}\n#[cfg(test)]\nfn arm() {\n    ON.set(true);\n}\n";
    assert!(check("crates/app/src/a.rs", local).is_empty());
    let reads = format!("{DECL}#[test]\nfn t() {{\n    assert!(ON.load(Relaxed));\n    let n = SEEN.lock().unwrap().len();\n    assert!(*SEEN.lock().unwrap() == vec![]);\n}}\n");
    assert!(check("crates/app/src/a_tests.rs", &reads).is_empty());
    let live = format!(
        "{DECL}/// The next id.\npub fn next() -> usize {{\n    N.fetch_add(1, Relaxed)\n}}\n"
    );
    assert!(check("crates/app/src/a.rs", &live).is_empty());
    let other = format!("{DECL}fn t() {{ ON.store(true, Relaxed); }}\n");
    assert!(check("tools/lint/src/a_tests.rs", &other).is_empty());
    assert!(check(
        "crates/app/src/a_tests.rs",
        "fn t() { ONE.store(1); NONE.set(2); }\n"
    )
    .is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = format!("{DECL}#[test]\nfn t() {{\n    // lint: allow(test-static-write) one-shot and idempotent: every test sets the same value\n    ON.store(true, Relaxed);\n}}\n");
    let line = check("crates/app/src/a_tests.rs", &src)[0];
    assert!(crate::rules::allowed(&src, line, RULE));
}
