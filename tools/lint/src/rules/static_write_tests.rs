use super::{check_file, decls, RULE};
use crate::code::bare_lines;

/// The lines `run` would flag in one file: its own statics are the names it knows.
fn check(path: &str, source: &str) -> Vec<usize> {
    let test = crate::rules::test_code::test_lines(path, source, false);
    let names = decls(&bare_lines(source)).process;
    check_file(path, source, &test, &names)
        .iter()
        .map(|f| f.line)
        .collect()
}

const DECL: &str = "static ON: AtomicBool = AtomicBool::new(false);\nstatic N: AtomicUsize = AtomicUsize::new(0);\nstatic SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());\n";

#[test]
fn flags_a_test_storing_to_a_crate_static() {
    let src = format!("{DECL}#[cfg(test)]\nmod tests {{\n    fn t() {{\n        ON.store(true, Relaxed);\n        super::N.fetch_add(1, Relaxed);\n        SEEN.lock().unwrap().push(3);\n        *SEEN.lock().unwrap() = Vec::new();\n    }}\n}}\n");
    assert_eq!(check("crates/app/src/a.rs", &src), [7, 8, 9, 10]);
    assert_eq!(
        check(
            "tests/a.rs",
            "static F: OnceLock<u8> = OnceLock::new();\nfn t() { F.set(1); }\n"
        ),
        [2]
    );
}

#[test]
fn flags_a_test_hook_that_writes_one() {
    let src = "static ARMED: AtomicBool = AtomicBool::new(false);\n\n/// Count every walk from now on. A test hook.\n#[doc(hidden)]\npub fn arm() {\n    ARMED.store(true, Relaxed);\n}\n";
    assert_eq!(check("crates/ecs/src/walks.rs", src), [6]);
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
