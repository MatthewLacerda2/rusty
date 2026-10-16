//! `Physics.*` point queries from script against the collider geometry (#311).

use super::spatial::spatial_runtime;

#[test]
fn point_queries_match_the_collider_geometry() {
    let (m, a, _) = spatial_runtime();
    // `a` spans [-1,1]³: from (5,0,0) the closest point is its +X face.
    assert_eq!(
        m.eval(&format!("Physics.ClosestPoint({a}, 5,0,0)"))
            .unwrap(),
        "true, 1, 0, 0"
    );
    assert_eq!(
        m.eval(&format!("Physics.ContainsPoint({a}, 0,0,0)"))
            .unwrap(),
        "true"
    );
    assert_eq!(
        m.eval(&format!("Physics.ContainsPoint({a}, 0,0,3)"))
            .unwrap(),
        "false"
    );
    // GetBounds surfaces the cached world AABB: [-1,1] on every axis.
    assert_eq!(
        m.eval(&format!("Physics.GetBounds({a})")).unwrap(),
        "true, -1, -1, -1, 1, 1, 1"
    );
    assert!(m
        .eval("Physics.GetBounds(9999)")
        .unwrap()
        .starts_with("false"));
}
