use super::{local_ops_named, ops_named, root_of};

#[test]
fn an_effect_module_reaches_the_ops_its_row_calls_by_bare_name() {
    let src = "pub fn set_intensity(vc: &mut V, v: f32) {}\n\
               pub fn set_preview(vc: &mut V, on: bool) {}\n\
               pub const EFFECT: Effect = Effect { field: effect!(bloom, |c, v: B| {\n    \
               set_intensity(c, v.intensity);\n}), knobs: &[Knob { set: set_intensity }] };\n\
               fn f(x: X) { x.set_preview(true); other::set_preview(); }\n";
    // Its own definitions, a method and another module's op are not calls.
    let reached: Vec<String> = local_ops_named(src, &[], "rendering::post_processing::bloom")
        .into_iter()
        .collect();
    assert_eq!(
        reached,
        ["rendering::post_processing::bloom::set_intensity"]
    );
    assert!(local_ops_named(src, &[false, false, false, true, true], "m").is_empty());
}

/// Inside `rusty-scene` the field tables spell `scene::authoring` as
/// `crate::authoring`; elsewhere that path names no op.
#[test]
fn a_file_of_the_scene_crate_names_ops_by_its_own_root() {
    let src = "use crate::authoring::{rendering::camera, physics::collider as c};\n\
               fn f() { camera::set_far(x, 1.0); c::set_active(y, true); }\n";
    let root = root_of("crates/scene/src/authoring/fields/kinds/render.rs");
    let named: Vec<String> = ops_named(src, &[], root).into_iter().collect();
    assert_eq!(
        named,
        [
            "physics::collider::set_active",
            "rendering::camera::set_far"
        ]
    );
    assert!(ops_named(src, &[], root_of("src/script/api/component.rs")).is_empty());
}

/// A file in another member crate (`api` and `editor`) names the ops
/// through `rusty_scene::authoring`, by a `use` or spelled whole.
#[test]
fn a_file_of_another_member_names_ops_through_the_scene_crate() {
    let src = "use rusty_scene::authoring::rendering::camera;\n\
               fn f() { camera::set_far(x, 1.0); \
               rusty_scene::authoring::physics::collider::set_active(y, true); }\n";
    let root = root_of("crates/api/src/component.rs");
    assert_eq!(root, "rusty_scene::authoring");
    let named: Vec<String> = ops_named(src, &[], root).into_iter().collect();
    assert_eq!(
        named,
        [
            "physics::collider::set_active",
            "rendering::camera::set_far"
        ]
    );
    assert_eq!(root_of("src/script/api/component.rs"), super::ROOT);
}
