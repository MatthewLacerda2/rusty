//! `Physics.Raycast` / `SphereCast` hit info and `Physics.RaycastAll` from
//! script (#446), over `tests_spatial`'s two boxes: `a` spanning [-1,1]³ on
//! layer 0 and `b` spanning z ∈ [9,11] on layer 3.

use super::tests_spatial::spatial_runtime;

#[test]
fn raycast_appends_the_hit_point_and_surface_normal() {
    let (m, a, _) = spatial_runtime();
    // From z=-5 along +Z: `a`'s -Z face at z=-1, 4 m away, facing back.
    assert_eq!(
        m.eval("Physics.Raycast(0,0,-5, 0,0,1)").unwrap(),
        format!("true, {a}, 4, 0, 0, -1, 0, 0, -1")
    );
    // A miss zeroes every value after `hit`.
    assert_eq!(
        m.eval("Physics.Raycast(0,0,-5, 0,0,-1)").unwrap(),
        "false, 0, 0, 0, 0, 0, 0, 0, 0"
    );
}

#[test]
fn three_value_callers_are_undisturbed() {
    let (m, a, _) = spatial_runtime();
    let three = m
        .eval("local hit, id, d = Physics.Raycast(0,0,-5, 0,0,1); return hit, id, d")
        .unwrap();
    assert_eq!(three, format!("true, {a}, 4"));
}

#[test]
fn sphere_cast_appends_the_contact_point_and_normal() {
    let (m, _, b) = spatial_runtime();
    // A 0.5 m sphere from z=2 meets `b`'s z=9 face after 6.5 m of travel.
    assert_eq!(
        m.eval("Physics.SphereCast(0,0,2, 0,0,1, 0.5)").unwrap(),
        format!("true, {b}, 6.5, 0, 0, 9, 0, 0, -1")
    );
}

#[test]
fn raycast_all_lists_every_hit_sorted_with_point_and_normal() {
    let (m, a, b) = spatial_runtime();
    let all = m
        .eval(
            "local t = Physics.RaycastAll(0,0,-5, 0,0,1, 100)
             return #t, t[1].id, t[1].distance, t[2].id, t[2].distance,
                    t[2].point.z, t[2].normal.x, t[2].normal.y, t[2].normal.z",
        )
        .unwrap();
    assert_eq!(all, format!("2, {a}, 4, {b}, 14, 9, 0, 0, -1"));
    // max_distance stops short of `b`.
    assert_eq!(
        m.eval("#Physics.RaycastAll(0,0,-5, 0,0,1, 10)").unwrap(),
        "1"
    );
}

#[test]
fn raycast_all_layer_mask_keeps_only_that_layer() {
    let (m, _, b) = spatial_runtime();
    let masked = m
        .eval("local t = Physics.RaycastAll(0,0,-5, 0,0,1, 100, 1 << 3); return #t, t[1].id")
        .unwrap();
    assert_eq!(masked, format!("1, {b}"));
    assert_eq!(
        m.eval("#Physics.RaycastAll(0,0,-5, 0,0,1, 100, 1 << 5)")
            .unwrap(),
        "0"
    );
}
