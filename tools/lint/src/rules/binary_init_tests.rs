use super::{check, RULE};

const INIT: &str = "fn main() {\n    rusty::init();\n    run();\n}\n";
const LOGGER: &str = "fn main() {\n    rusty::logging::init();\n    run();\n}\n";

#[test]
fn flags_a_binary_that_starts_without_init() {
    for root in ["src/main.rs", "src/bin/new_tool.rs", "src/bin/tool/main.rs"] {
        let found: Vec<_> = check(root, LOGGER)
            .iter()
            .map(|f| (f.line, f.rule))
            .collect();
        assert_eq!(found, [(1, RULE)], "{root}");
    }
    // Called, but not first; or only in a comment or a string.
    let late = "fn main() {\n    let a = 1;\n    rusty::init();\n}\n";
    assert_eq!(check("src/bin/a.rs", late).len(), 1);
    let quoted = "fn main() {\n    // rusty::init();\n    let _ = \"rusty::init();\";\n}\n";
    assert_eq!(check("src/bin/a.rs", quoted).len(), 1);
}

#[test]
fn passes_a_binary_that_inits_first_and_files_that_are_not_binaries() {
    for root in ["src/main.rs", "src/bin/play.rs", "src/bin/tool/main.rs"] {
        assert!(check(root, INIT).is_empty(), "{root}");
    }
    let commented = "fn main() {\n\n    rusty::init(); // the logger and the check\n}\n";
    assert!(check("src/bin/a.rs", commented).is_empty());
    for other in [
        "src/lib.rs",
        "src/bin/tool/helper.rs",
        "crates/dev/src/a.rs",
        "tests/a.rs",
    ] {
        assert!(check(other, LOGGER).is_empty(), "{other}");
    }
    // A binary root with no `fn main` of its own (a module file) is not judged.
    assert!(check("src/bin/a.rs", "pub fn helper() {}\n").is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_a_binary() {
    let src = "// lint: allow(binary-init) a probe that must not hash engine/\nfn main() {}\n";
    let found = check("src/bin/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
