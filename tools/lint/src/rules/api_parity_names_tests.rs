use super::local_ops_named;

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
