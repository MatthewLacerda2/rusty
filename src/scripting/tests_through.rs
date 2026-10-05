//! `Physics.RaycastThrough` from script (#830), over `tests_spatial`'s two
//! boxes: `a` spanning [-1,1]³ on layer 0 and `b` spanning z ∈ [9,11] on layer 3.

use super::tests_spatial::spatial_runtime;

#[test]
fn raycast_through_pairs_each_entry_with_its_exit() {
    let (m, a, b) = spatial_runtime();
    let script = "local t = Physics.RaycastThrough(0,0,-5, 0,0,1, 100)
                  return #t, t[1].id, t[1].root, t[1].bone, t[1].enter.distance,
                         t[1].exit.distance, t[1].thickness, t[1].exit.normal.z,
                         t[2].id, t[2].enter.point.z, t[2].exit.point.z";
    assert_eq!(
        m.eval(script).unwrap(),
        format!("2, {a}, {a}, nil, 4, 6, 2, 1, {b}, 9, 11")
    );
}

#[test]
fn raycast_through_ending_inside_has_no_exit() {
    let (m, _, b) = spatial_runtime();
    let script = "local t = Physics.RaycastThrough(0,0,-5, 0,0,1, 15)
                  return #t, t[2].id, t[2].exit, t[2].thickness";
    assert_eq!(m.eval(script).unwrap(), format!("2, {b}, nil, 1"));
}

#[test]
fn raycast_through_layer_mask_keeps_only_that_layer() {
    let (m, _, b) = spatial_runtime();
    let masked = m
        .eval("local t = Physics.RaycastThrough(0,0,-5, 0,0,1, 100, 1 << 3); return #t, t[1].id")
        .unwrap();
    assert_eq!(masked, format!("1, {b}"));
}
