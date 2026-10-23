//! `Physics.*` point queries from script against the collider geometry (#311).

use super::spatial::{runtime_over, spatial_runtime};
use crate::physics::turned_hull;

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

#[test]
fn a_turned_hull_holds_its_own_corners() {
    // #1211: parry's projection answered a corner of this hull 66.8 away.
    let (scene, id, corners) = turned_hull();
    let m = runtime_over(scene);
    for c in corners {
        let at = format!("{id}, {}, {}, {}", c.x, c.y, c.z);
        let closest = m.eval(&format!("Physics.ClosestPoint({at})")).unwrap();
        let xyz: Vec<f32> = (closest.split(", ").skip(1))
            .map(|v| v.parse().unwrap())
            .collect();
        let gap = (glam::Vec3::from_slice(&xyz) - c).length();
        assert!(gap < 0.01, "{c}: ClosestPoint answered {closest}");
        let inside = m.eval(&format!("Physics.ContainsPoint({at})")).unwrap();
        assert_eq!(inside, "true", "{c}");
    }
}
