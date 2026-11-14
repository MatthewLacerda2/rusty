use super::{defined, resolved, unheld, Rule};
use crate::rules::sanitize::check;

const RULES: &str = "crates/scene/src/rules/sanitize.rs";
const SETTER: &str = "src/scene/authoring/a.rs";

/// The rules `source` defines, resolved as if it were the rules folder's one file.
fn of(source: &str) -> Vec<Rule> {
    resolved([(RULES, source)].into_iter())
}

fn held(rules: &[Rule], name: &str) -> Vec<usize> {
    let rule = rules.iter().find(|r| r.name == name).expect("defined");
    rule.held.clone()
}

const FINITE_OR: &str =
    "pub fn finite_or(v: f32, fallback: f32) -> f32 {\n    if v.is_finite() {\n        v\n    } else {\n        fallback\n    }\n}\n";

#[test]
fn a_rule_holds_what_it_tests_or_hands_on_and_no_fallback() {
    let wrapper = "pub fn finite(v: f32) -> f32 {\n    finite_or(v, 0.0)\n}\n";
    let rules = of(&format!("{FINITE_OR}{wrapper}"));
    assert_eq!(
        held(&rules, "finite_or"),
        [0],
        "the fallback is passed through"
    );
    assert_eq!(
        held(&rules, "finite"),
        [0],
        "through finite_or's held value"
    );
    assert!(unheld(&rules)
        .iter()
        .all(|f| !f.message.contains("`finite`")));
}

#[test]
fn a_pass_through_rule_is_flagged_and_vouches_for_nothing() {
    let through = "pub fn hold_it(v: f32) -> f32 {\n    v\n}\n";
    let rules = of(through);
    let found = unheld(&rules);
    assert_eq!((found.len(), found[0].line), (1, 1));
    assert!(
        found[0].message.contains("`hold_it`"),
        "{}",
        found[0].message
    );
    let setter = "pub fn set_a(c: &mut C, v: f32) {\n    c.a = hold_it(v);\n}\n";
    assert_eq!(check(SETTER, setter, &[], &rules).len(), 1);
}

#[test]
fn a_setter_counts_only_at_a_held_position() {
    let rules = of(FINITE_OR);
    let value = "pub fn set_a(c: &mut C, v: f32) {\n    c.a = finite_or(v, 1.0);\n}\n";
    assert!(check(SETTER, value, &[], &rules).is_empty());
    let fallback = "pub fn set_a(c: &mut C, v: f32) {\n    c.a = finite_or(c.a, v);\n}\n";
    assert_eq!(check(SETTER, fallback, &[], &rules).len(), 1);
}

#[test]
fn only_bare_pub_fns_before_the_tests_are_rules() {
    let src = "pub(crate) fn inner(v: f32) -> f32 {\n    v\n}\nfn private(v: f32) -> f32 {\n    v\n}\n\
               pub const fn at(v: f32) -> f32 {\n    v\n}\n#[cfg(test)]\nmod tests {\n    pub fn t() {}\n}\n";
    let names: Vec<String> = defined(RULES, src).into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["at"]);
    let elsewhere = [("crates/scene/src/authoring/a.rs", FINITE_OR)];
    assert!(
        resolved(elsewhere.into_iter()).is_empty(),
        "outside the rules folder"
    );
}

#[test]
fn a_rule_with_no_float_or_a_held_field_test_passes() {
    let mesh = "pub fn hold_mesh(mesh: &mut MeshComponent) -> Option<String> {\n    None\n}\n";
    let range = "pub fn range_or(r: Range, fallback: Range) -> Range {\n    if r.min.is_finite() {\n        r\n    } else {\n        fallback\n    }\n}\n";
    let rules = of(&format!("{mesh}{range}"));
    assert_eq!(held(&rules, "range_or"), [0]);
    assert!(unheld(&rules).is_empty(), "holding the value is enough");
}

#[test]
fn an_excused_rule_holds_its_first_float() {
    let src = "/// Doc.\n// lint: allow(sanitize) each arm tests its colour\npub fn optional(v: Option<Vec3>, current: Option<Vec3>) -> Option<Vec3> {\n    match v {\n        Some(c) => Some(c),\n        None => current,\n    }\n}\n";
    let rules = of(src);
    assert_eq!(held(&rules, "optional"), [0]);
    assert!(unheld(&rules).is_empty());
}
