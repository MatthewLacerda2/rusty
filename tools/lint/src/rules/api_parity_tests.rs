use super::{check, ops_named, RULE};
use std::collections::{BTreeMap, BTreeSet};

fn named(src: &str) -> Vec<String> {
    ops_named(src, &[]).into_iter().collect()
}

#[test]
fn resolves_aliases_trees_and_table_paths() {
    let src = "use crate::scene::authoring::{character_controller as cc, collider};\n\
               use crate::scene::authoring::nav::{agent, obstacle as ob};\n\
               use crate::scene::authoring::text::{self as ops, FontSlot};\n\
               use crate::scene::{authoring, Scene};\n\
               fn f() {\n    cc::set_height(c, 1.0);\n    op!(\"is_trigger\", collider::set_trigger),\n    \
               agent::set_speed(a, 2.0); ob::set_carve(o, true);\n    ops::set_font(t, s);\n    \
               authoring::rigidbody::set_mass(r, 5.0);\n    \
               crate::scene::authoring::camera::set_fov(c, 60.0);\n    other::set_x(1);\n}\n";
    assert_eq!(
        named(src),
        [
            "camera::set_fov",
            "character_controller::set_height",
            "collider::set_trigger",
            "nav::agent::set_speed",
            "nav::obstacle::set_carve",
            "rigidbody::set_mass",
            "text::set_font",
        ]
    );
}

#[test]
fn skips_test_lines_and_comments() {
    let src = "use crate::scene::authoring::rigidbody as rb;\n// rb::set_mass(r, 1.0)\nrb::set_kinematic(r, true);\n";
    assert_eq!(named(src), ["rigidbody::set_kinematic"]);
    assert!(ops_named(src, &[false, false, true]).is_empty());
}

fn set(ops: &[&str]) -> BTreeSet<String> {
    ops.iter().map(|s| s.to_string()).collect()
}

const RB: &str = "src/scene/authoring/rigidbody.rs";

fn findings(editor: &[&str], api: &[&str], src: &str) -> Vec<(usize, bool)> {
    let authoring = BTreeMap::from([(RB.to_string(), src)]);
    check(&set(editor), &set(api), &authoring)
        .iter()
        .map(|f| (f.line, f.message.starts_with("stale")))
        .collect()
}

#[test]
fn flags_an_op_the_editor_names_and_lua_does_not() {
    let src = "pub fn set_kinematic() {}\npub fn set_mass() {}\n";
    assert_eq!(findings(&["rigidbody::set_mass"], &[], src), [(2, false)]);
    let authoring = BTreeMap::from([(RB.to_string(), src)]);
    let f = &check(&set(&["rigidbody::set_mass"]), &set(&[]), &authoring)[0];
    assert_eq!((f.path.as_str(), f.rule), (RB, RULE));
}

#[test]
fn passes_a_reached_op_and_an_explained_one() {
    let src = "pub fn set_kinematic() {}\n// lint: allow(api-parity) a display toggle\npub fn set_mass() {}\n";
    let ed = ["rigidbody::set_kinematic", "rigidbody::set_mass"];
    assert!(findings(&ed, &["rigidbody::set_kinematic"], src).is_empty());
}

#[test]
fn flags_a_stale_allow() {
    let src = "// lint: allow(api-parity) a display toggle\npub fn set_mass() {}\n";
    let reached = ["rigidbody::set_mass"];
    assert_eq!(findings(&reached, &reached, src), [(1, true)]);
    assert_eq!(
        findings(&[], &[], src),
        [(1, true)],
        "no inspector names it"
    );
}

#[test]
fn finds_an_op_reexported_from_a_folder_file() {
    let fx = BTreeMap::from([
        (
            "src/scene/authoring/particles/mod.rs".to_string(),
            "pub use render::*;\n",
        ),
        (
            "src/scene/authoring/particles/render.rs".to_string(),
            "\npub fn set_blend() {}\n",
        ),
    ]);
    let f = check(&set(&["particles::set_blend"]), &set(&[]), &fx);
    assert_eq!(
        (f[0].path.as_str(), f[0].line),
        ("src/scene/authoring/particles/render.rs", 2)
    );
}
