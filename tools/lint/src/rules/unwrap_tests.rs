use super::unwrap::{check, missing_roots, DENY, RULE};

fn sim() -> Vec<String> {
    vec!["src/scene".to_string(), "src/physics".to_string()]
}

#[test]
fn a_sim_root_with_the_deny_passes() {
    let src = format!("//! scene\n\n{DENY}\n\nmod a;\n");
    assert!(check("src/scene/mod.rs", &src, &sim()).is_empty());
}

#[test]
fn a_sim_root_without_the_deny_is_flagged() {
    let found = check(
        "src/scene/mod.rs",
        "mod a;\n// #![deny(clippy::unwrap_used)]\n",
        &sim(),
    );
    assert_eq!(found.len(), 1, "a commented-out deny does not count");
    assert_eq!((found[0].line, found[0].rule), (1, RULE));
}

#[test]
fn a_sim_file_lifting_the_ban_is_flagged() {
    for attr in [
        "#[allow(clippy::unwrap_used)]",
        "#[expect(clippy::unwrap_used, reason = \"x\")]",
        "#![allow(clippy::restriction)]",
    ] {
        let found = check("src/physics/world/step.rs", attr, &sim());
        assert_eq!(found.len(), 1, "{attr}");
    }
}

#[test]
fn platform_files_and_prose_pass() {
    let allow = "#![allow(clippy::unwrap_used)]";
    assert!(check("src/render/mod.rs", allow, &sim()).is_empty());
    assert!(check("src/shell/mod.rs", "mod a;", &sim()).is_empty());
    assert!(check("src/scenery/mod.rs", "mod a;", &sim()).is_empty());
    let prose = "// allow(clippy::unwrap_used) is banned here\nlet x = clippy::unwrap_used_ok;";
    assert!(check("src/scene/a.rs", prose, &sim()).is_empty());
}

#[test]
fn a_sim_row_without_a_root_is_flagged() {
    let files = vec!["src/scene/mod.rs".to_string()];
    let found = missing_roots(&files, &sim());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "src/physics/mod.rs");
}
