use super::{check, helpers, RULE};

const PATH: &str = "src/scene/authoring/a.rs";

fn found(src: &str) -> Vec<usize> {
    let helpers = helpers("pub(super) fn finite(v: f32) -> f32 {\n    v\n}\n");
    check(PATH, src, &[], &helpers)
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_a_raw_float_or_vector_and_a_single_component() {
    let raw = "pub fn set_a(c: &mut C, v: f32) {\n    c.a = v.max(0.0);\n}\n";
    assert_eq!(found(raw), [1]);
    let vector = "/// doc\npub fn set_b(c: &mut C,\n    rgb: glam::Vec3,\n) {\n    c.b = rgb;\n}\n";
    assert_eq!(found(vector), [2]);
    let component = "pub fn set_c(c: &mut C, v: Vec2) {\n    c.x = finite(v.x);\n}\n";
    assert_eq!(found(component), [1]);
    let helpers = helpers("pub(super) fn finite(v: f32) -> f32 {\n    v\n}\n");
    let f = &check(PATH, raw, &[], &helpers)[0];
    assert_eq!(f.rule, RULE);
}

#[test]
fn passes_a_helper_a_delegation_and_non_floats() {
    let helper = "pub fn set_a(c: &mut C, v: f32) {\n    c.a = finite(v);\n}\n";
    assert!(found(helper).is_empty());
    let pair = "pub fn set_ab(c: &mut C, a: f32, b: f32) {\n    c.a = finite(a);\n    \
                c.b = finite(b);\n}\npub fn set_a(c: &mut C, a: f32) {\n    set_ab(c, a, c.b);\n}\n";
    assert!(found(pair).is_empty());
    let flag = "pub fn set_on(c: &mut C, on: bool, n: u32) {\n    c.on = on;\n}\n";
    assert!(found(flag).is_empty());
    let private = "fn set_a(c: &mut C, v: f32) {\n    c.a = v;\n}\n";
    assert!(found(private).is_empty());
}

#[test]
fn other_dirs_and_test_code_are_not_scanned() {
    let raw = "pub fn set_a(c: &mut C, v: f32) {\n    c.a = v;\n}\n";
    assert!(check("src/render/a.rs", raw, &[], &[]).is_empty());
    assert!(check(PATH, raw, &[true, true, true], &[]).is_empty());
}

#[test]
fn helpers_skip_the_test_module_and_an_allow_excuses() {
    let src =
        "pub(super) fn unit(v: f32) -> f32 {\n}\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
    assert_eq!(helpers(src), ["unit"]);
    let src = "// lint: allow(sanitize) unbounded by design\npub fn set_a(c: &mut C, v: f32) {\n    c.a = v;\n}\n";
    let f = check(PATH, src, &[], &[]);
    assert!(crate::rules::allowed(src, f[0].line, RULE));
}

#[test]
fn any_pub_visibility_and_float_carrying_types_are_checked() {
    let scoped = "pub(crate) fn set_a(c: &mut C, v: f32) {\n    c.a = v;\n}\n";
    assert_eq!(found(scoped), [1]);
    for ty in [
        "[f32; 3]",
        "f64",
        "Option<f32>",
        "Vec<f32>",
        "&[Vec3]",
        "Range",
        "core::curve::ColorRange",
    ] {
        let src = format!("pub fn set_a(c: &mut C, v: {ty}) {{\n    c.a = v;\n}}\n");
        assert_eq!(found(&src), [1], "{ty}");
    }
    let other = "pub fn set_a(c: &mut C, v: Option<String>, w: &[u32]) {\n    c.a = v;\n}\n";
    assert!(found(other).is_empty());
}
