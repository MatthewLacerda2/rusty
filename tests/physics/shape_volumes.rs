//! The oriented volumes (#1881): an `OverlapBox` / `CheckBox` with an
//! orientation and `CheckCapsule`, each with a hit and a miss, plus the
//! argument errors every shape query shares. The corridor is `shape_casts`'.

use super::shape_casts::{corridor, eval};

#[test]
fn an_oriented_overlap_box_catches_what_the_axis_aligned_one_misses() {
    let (game, near, _) = corridor();
    // A plank 3 m long along z, held at x = 2.2 beside `near`; turned 90°
    // about y it spans x 0.7..3.7.
    let plank = "Physics.OverlapBox(2.2,0,10, 0.2,0.2,1.5";
    assert_eq!(eval(&game, &format!("return #{plank})")), "0");
    assert_eq!(eval(&game, &format!("return #{plank}, nil,nil,nil)")), "0");
    let turned = format!("local t = {plank}, 0,90,0) return #t, t[1]");
    assert_eq!(eval(&game, &turned), format!("1, {near}"));
    let check = "return Physics.CheckBox(2.2,0,10, 0.2,0.2,1.5), \
                 Physics.CheckBox(2.2,0,10, 0.2,0.2,1.5, 0,90,0)";
    assert_eq!(eval(&game, check), "false, true");
}

#[test]
fn check_capsule_is_true_inside_a_collider_and_false_beside_it() {
    let (game, _, _) = corridor();
    let check = |x: f32| {
        eval(
            &game,
            &format!("Physics.CheckCapsule({x},0,9.5, {x},0,10.5, 0.2)"),
        )
    };
    assert_eq!(check(0.0), "true");
    assert_eq!(check(3.0), "false");
}

#[test]
fn a_bad_size_direction_or_orientation_is_an_error_naming_it() {
    let (game, _, _) = corridor();
    let error = |call: &str| {
        let line = format!(
            "local ok, e = pcall(function() return Physics.{call} end) return ok, tostring(e)"
        );
        eval(&game, &line)
    };
    for (call, named) in [
        ("CapsuleCast(0,0,0, 0,2,0, 0, 0,0,1)", "CapsuleCast: radius"),
        ("BoxCast(0,0,0, 1,-1,1, 0,0,1)", "BoxCast: half_extents"),
        ("BoxCast(0,0,0, 1,1,1, 0,0,0)", "BoxCast: direction"),
        ("SphereCast(0,0,0, 0,0,0, 0.5)", "SphereCast: direction"),
        (
            "SphereCastAll(0/0,0,0, 0,0,1, 0.5, 9)",
            "SphereCastAll: origin",
        ),
        ("OverlapBox(0,0,0, 1,1,1, 0,90)", "OverlapBox: orientation"),
        ("CheckCapsule(0,0,0, 0,1,0, -1)", "CheckCapsule: radius"),
    ] {
        let got = error(call);
        assert!(
            got.starts_with("false") && got.contains(named),
            "{call}: {got}"
        );
    }
}
