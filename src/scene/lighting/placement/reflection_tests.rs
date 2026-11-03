//! Reflection probes only on reachable floor (#1737).

use super::test_sites::{plan_on_nav, roofed_house, yard};
use super::ReflectionPlan;

#[test]
fn an_unreachable_roof_gets_no_probe() {
    let mut scene = yard();
    roofed_house(&mut scene);
    let plans = plan_on_nav(&scene);
    assert!(!plans.is_empty());
    for p in &plans {
        assert!(p.position.y < 3.0, "a probe on the roof: {p:?}");
    }
    let inside = |p: &&ReflectionPlan| p.position.x.abs() < 5.0 && p.position.z.abs() < 5.0;
    assert!(
        plans.iter().any(|p| inside(&p)),
        "the room has its own probe"
    );
}
