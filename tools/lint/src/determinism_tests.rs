use super::*;

fn roots() -> Vec<String> {
    vec!["src/shell/mod.rs".to_string()]
}

const ALLOW: &str = "#![allow(clippy::disallowed_methods, clippy::disallowed_types)]";

#[test]
fn a_platform_root_may_opt_out() {
    assert!(opt_out_violations("src/shell/mod.rs", ALLOW, &roots()).is_empty());
}

#[test]
fn a_sim_module_may_not_opt_out() {
    let v = opt_out_violations("src/physics/mod.rs", ALLOW, &roots());
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v[0].starts_with("OPT_OUT src/physics/mod.rs:1"));
}

#[test]
fn a_platform_file_below_its_root_may_not_opt_out() {
    let code = "#[allow(clippy::disallowed_methods)]\nfn f() {}";
    assert_eq!(
        opt_out_violations("src/shell/frame.rs", code, &roots()).len(),
        1
    );
}

#[test]
fn allowing_a_group_that_holds_the_lints_is_an_opt_out() {
    for code in ["#![allow(clippy::style)]", "#[expect(clippy::all)]"] {
        assert_eq!(
            opt_out_violations("src/app/mod.rs", code, &roots()).len(),
            1
        );
    }
}

#[test]
fn comments_and_longer_names_are_not_opt_outs() {
    let code = "// see clippy::disallowed_methods\nlet x = clippy::all_things;";
    assert!(opt_out_violations("src/app/mod.rs", code, &roots()).is_empty());
}

#[test]
fn the_shipped_config_bans_everything() {
    let clippy = include_str!("../../../clippy.toml");
    let cargo = include_str!("../../../Cargo.toml");
    assert_eq!(config_violations(clippy, cargo), Vec::<String>::new());
}

#[test]
fn a_dropped_ban_or_deny_is_reported() {
    let clippy = "disallowed-methods = [{ path = \"std::time::Instant::now\" }]";
    let v = config_violations(clippy, "disallowed_methods = \"deny\"");
    assert!(v.iter().any(|s| s.contains("`std::time::SystemTime`")));
    assert!(v
        .iter()
        .any(|s| s.starts_with("UNDENIED `disallowed_types")));
    assert!(!v.iter().any(|s| s.contains("`std::time::Instant::now`")));
}

#[test]
fn a_reasoned_statement_opt_out_passes_and_a_bare_one_fails() {
    let expect = "    #[expect(\n        clippy::disallowed_methods,\n        reason = \"r\"\n    )]\n    std::env::set_current_dir(root)?;\n";
    let excused = format!("    // lint: allow(statement-opt-out) the cwd move\n{expect}");
    assert!(opt_out_violations("src/core/project/mod.rs", &excused, &roots()).is_empty());
    let bare = opt_out_violations("src/core/project/mod.rs", expect, &roots());
    assert_eq!(bare.len(), 1, "{bare:?}");
    let unreasoned = format!("    // lint: allow(statement-opt-out)\n{expect}");
    assert_eq!(
        opt_out_violations("src/core/project/mod.rs", &unreasoned, &roots()).len(),
        1
    );
}

#[test]
fn the_ban_keeps_process_state_on_the_list() {
    let clippy = include_str!("../../../clippy.toml").replace("std::env::set_var", "x");
    let v = config_violations(&clippy, include_str!("../../../Cargo.toml"));
    assert_eq!(
        v,
        vec!["UNBANNED `std::env::set_var` — clippy.toml no longer lists it"]
    );
}
