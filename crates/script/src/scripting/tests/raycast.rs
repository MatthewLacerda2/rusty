use super::spatial::spatial_runtime;

#[test]
fn raycast_appends_the_hit_point_and_surface_normal() {
    let (m, a, _) = spatial_runtime();
    assert_eq!(
        m.eval("Physics.Raycast(0,0,-5, 0,0,1)").unwrap(),
        format!("true, {a}, 4, 0, 0, -1, 0, 0, -1, nil, nil, {a}, nil")
    );
    assert_eq!(
        m.eval("Physics.Raycast(0,0,-5, 0,0,-1)").unwrap(),
        "false, 0, 0, 0, 0, 0, 0, 0, 0, nil, nil, nil, nil"
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
    // A shape cast's distance and point are not exact at the face, so they compare within 1e-4.
    let script = "local hit, id, d, px, py, pz, nx, ny, nz, bone, name, root = \
                  Physics.SphereCast(0,0,2, 0,0,1, 0.5) \
                  local function near(x, want) return math.abs(x - want) < 1e-4 end \
                  return hit, id, near(d, 6.5), near(px, 0), near(py, 0), near(pz, 9), \
                  nx, ny, nz, bone, name, root";
    assert_eq!(
        m.eval(script).unwrap(),
        format!("true, {b}, true, true, true, true, 0, 0, -1, nil, nil, {b}")
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
