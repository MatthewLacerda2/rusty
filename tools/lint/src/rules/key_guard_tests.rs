use super::{check, RULE};

const LINKS: &str = "[dependencies]\nrusty-dev = { workspace = true, optional = true }\n";

#[test]
fn flags_a_package_linking_rusty_dev_without_the_guard() {
    let found = check(
        "crates/shell/Cargo.toml",
        LINKS,
        ["fn f() {}\n"].into_iter(),
    );
    let found = found.expect("a finding");
    assert_eq!(
        (found.path.as_str(), found.rule),
        ("crates/shell/Cargo.toml", RULE)
    );
    // A comment or the macro's definition is no expansion.
    let not = [
        "// rusty_dev::no_provider_keys!();\n",
        "macro_rules! no_provider_keys {}\n",
    ];
    assert!(check("Cargo.toml", LINKS, not.into_iter()).is_some());
    // A target table's entry and a dev-dependency link it into the tests too.
    let target = "[target.'cfg(unix)'.dependencies]\nrusty-dev = { workspace = true }\n";
    assert!(check("Cargo.toml", target, std::iter::empty()).is_some());
    let dev = "[dev-dependencies]\nrusty-dev = { workspace = true }\n";
    assert!(check("crates/ui/Cargo.toml", dev, std::iter::empty()).is_some());
}

#[test]
fn passes_a_guarded_package_and_one_that_does_not_link_it() {
    let guarded = ["mod provider_keys {\n    rusty_dev::no_provider_keys!();\n}\n"];
    assert!(check("crates/shell/Cargo.toml", LINKS, guarded.into_iter()).is_none());
    let other = "[dependencies]\nrusty-core = { workspace = true }\n\
        [dev-dependencies]\nrusty-devtools = { workspace = true }\n";
    assert!(check("crates/ui/Cargo.toml", other, std::iter::empty()).is_none());
}
