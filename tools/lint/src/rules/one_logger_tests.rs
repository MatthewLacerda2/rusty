use super::{check, RULE};

const LOGGING: &str = "[dependencies]\nrusty-logging = { workspace = true }\n";
const BELOW: &str = "[dependencies]\nrusty-core = { workspace = true }\n";

/// The lines `source` is flagged on, in a package naming `rusty-logging`.
fn flagged(source: &str) -> Vec<usize> {
    check("crates/dev/src/a.rs", LOGGING, source)
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_test_support_s_capture_in_a_package_naming_rusty_logging() {
    let source = "fn t() {\n    rusty_test_support::logs::init();\n}\n";
    let found = check("crates/dev/src/a.rs", LOGGING, source);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].line, found[0].rule), (2, RULE));
    assert_eq!(flagged("use rusty_test_support::{logs, temp};\n"), [1]);
    let dev = "[dev-dependencies]\nrusty-logging = { workspace = true }\n";
    assert_eq!(check("crates/ui/src/a.rs", dev, source).len(), 1);
}

#[test]
fn flags_a_use_split_over_lines_an_alias_and_a_glob() {
    let split = "use rusty_test_support::{\n    logs,\n    temp,\n};\n";
    assert_eq!(flagged(split), [1]);
    let alias = "use rusty_test_support as ts;\nfn t() {\n    ts::logs::init();\n}\n";
    assert_eq!(flagged(alias), [3]);
    assert_eq!(flagged("use rusty_test_support::*;\n"), [1]);
    assert_eq!(
        flagged("use rusty_test_support::logs::{init, take};\n"),
        [1]
    );
}

#[test]
fn passes_the_engine_s_capture_and_a_package_below_rusty_logging() {
    let engine =
        "fn t() {\n    rusty_logging::init_for_tests();\n    rusty_test_support::temp::dir();\n}\n";
    assert!(flagged(engine).is_empty());
    let below = "fn t() {\n    rusty_test_support::logs::init();\n}\n";
    assert!(check("crates/asset/src/a.rs", BELOW, below).is_empty());
    assert!(flagged("// rusty_test_support::logs::init() would race here\n").is_empty());
    // Another crate's `test_support`, and a name that only starts with `logs`.
    let other = "use rusty_navigation::test_support::{logs, grid};\n";
    assert!(flagged(other).is_empty());
    assert!(flagged("use rusty_test_support::{logs_dir, temp};\n").is_empty());
}
