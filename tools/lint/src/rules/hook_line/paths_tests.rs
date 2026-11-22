use super::{expand, resolved};
use crate::code::code_lines;

fn paths(source: &str) -> Vec<(usize, String)> {
    let code = code_lines(source);
    resolved(&code, &vec![false; code.len()])
}

fn has(source: &str, path: &str) -> bool {
    paths(source).iter().any(|(_, p)| p == path)
}

#[test]
fn a_use_tree_expands_to_each_path_it_imports() {
    let found = expand("rusty_a::{b, c::{d as e, self}, f::*}");
    let want = [
        ("rusty_a::b", "b"),
        ("rusty_a::c::d", "e"),
        ("rusty_a::c", "c"),
    ];
    let want: Vec<(String, String)> = want
        .iter()
        .map(|(p, l)| (p.to_string(), l.to_string()))
        .collect();
    assert_eq!(found, want);
}

#[test]
fn a_name_resolves_through_the_file_s_imports() {
    let wrapped = "use rusty_lighting::{\n    occlusion::x::probe,\n    Bake,\n};\nfn f() {\n    probe();\n    Bake::new();\n}\n";
    assert!(has(wrapped, "rusty_lighting::occlusion::x::probe"));
    assert!(has(wrapped, "rusty_lighting::Bake::new"));
    let module = "use rusty::lighting::occlusion;\nfn f() { occlusion::x::probe(); }\n";
    assert!(has(module, "rusty_lighting::occlusion::x::probe"));
    let aliased = "use rusty_lighting::occlusion as occ;\nfn f() { occ::probe(); }\n";
    assert!(has(aliased, "rusty_lighting::occlusion::probe"));
}

#[test]
fn the_file_s_own_paths_and_re_exports_reach_no_other_crate() {
    let own = "use rusty_lighting::Bake;\nfn f() { crate::bake::run(); self::x::run(); }\n";
    let found = paths(own);
    assert!(found.iter().all(|(_, p)| !p.contains("run")), "{found:?}");
    let re_export = "pub use rusty_lighting::occlusion::probe;\n";
    assert!(paths(re_export).is_empty());
    let scoped = "pub(crate) use rusty_lighting::probe;\n";
    assert!(paths(scoped).is_empty());
    let local = "fn f() { bake::run(); }\n";
    assert!(
        has(local, "bake::run"),
        "an unresolved path stays as written"
    );
}

#[test]
fn test_lines_name_nothing() {
    let source = "use rusty_lighting::probe;\nfn f() { probe(); }\n";
    let code = code_lines(source);
    assert!(resolved(&code, &[true, true]).is_empty());
}
