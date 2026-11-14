use super::*;

fn args(numbers: &[f64]) -> MultiValue {
    MultiValue::from_vec(numbers.iter().map(|&x| mlua::Value::Number(x)).collect())
}

/// Six numbers: Unity's defaults, white for one tick.
#[test]
fn six_numbers_are_a_white_one_tick_line() {
    let drawn = parse("Debug.DrawLine", args(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0])).expect("valid");
    assert_eq!(drawn.a, Vec3::new(0.0, 1.0, 2.0));
    assert_eq!(drawn.b, Vec3::new(3.0, 4.0, 5.0));
    assert_eq!((drawn.color, drawn.ticks), ([1.0; 4], 1));
}

/// A colour, then a duration in seconds of sim time, counted in 60 Hz ticks.
#[test]
fn a_colour_and_a_duration_follow() {
    let mut n = vec![0.0; 6];
    n.extend([1.0, 0.0, 0.0, 0.5]);
    let drawn = parse("Debug.DrawRay", args(&n)).expect("valid");
    assert_eq!((drawn.color, drawn.ticks), ([1.0, 0.0, 0.0, 0.5], 1));
    n.push(2.0);
    assert_eq!(parse("Debug.DrawRay", args(&n)).expect("valid").ticks, 120);
}

/// A partial colour, or a non-number, raises and names the verb.
#[test]
fn a_wrong_shape_raises() {
    let err = parse("Debug.DrawLine", args(&[0.0; 8])).expect_err("8 numbers");
    assert!(err.to_string().contains("Debug.DrawLine"), "{err}");
    let mut v = args(&[0.0; 6]).into_vec();
    v[2] = mlua::Value::Boolean(true);
    let err = parse("Debug.DrawRay", MultiValue::from_vec(v)).expect_err("a bool");
    assert!(err.to_string().contains("argument 3"), "{err}");
}

/// Through the VM: a ray lands on the scene as origin..origin + dir.
#[test]
fn a_ray_reaches_the_scene() {
    let s = crate::session::Session::new("").expect("an empty session boots");
    s.eval("Debug.DrawRay(1, 2, 3, 0, 0, -4, 0, 1, 0, 1, 0.5)")
        .expect("DrawRay runs");
    let scene = s.world().scene().borrow();
    let line = *scene.debug_lines.lines().next().expect("one line");
    assert_eq!(
        (line.from, line.to),
        (Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, -1.0))
    );
    assert_eq!((line.color, line.ticks), ([0.0, 1.0, 0.0, 1.0], 30));
}
