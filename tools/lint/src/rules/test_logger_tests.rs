use super::{check, RULE};

const LOGGING: &str = "[dependencies]\nrusty-logging = { workspace = true }\n";
const BELOW: &str = "[dependencies]\nrusty-core = { workspace = true }\n";

#[test]
fn flags_test_support_s_capture_in_a_package_naming_rusty_logging() {
    let source = "fn t() {\n    rusty_test_support::logs::init();\n}\n";
    let found = check("crates/dev/src/a.rs", LOGGING, source);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].line, found[0].rule), (2, RULE));
    let listed = "use rusty_test_support::{logs, temp};\n";
    assert_eq!(check("crates/dev/src/a.rs", LOGGING, listed).len(), 1);
    let dev = "[dev-dependencies]\nrusty-logging = { workspace = true }\n";
    assert_eq!(check("crates/ui/src/a.rs", dev, source).len(), 1);
}

#[test]
fn passes_the_engine_s_capture_and_a_package_below_rusty_logging() {
    let engine =
        "fn t() {\n    rusty_logging::init_for_tests();\n    rusty_test_support::temp::dir();\n}\n";
    assert!(check("crates/dev/src/a.rs", LOGGING, engine).is_empty());
    let below = "fn t() {\n    rusty_test_support::logs::init();\n}\n";
    assert!(check("crates/asset/src/a.rs", BELOW, below).is_empty());
    let comment = "// rusty_test_support::logs::init() would race here\n";
    assert!(check("crates/dev/src/a.rs", LOGGING, comment).is_empty());
}
