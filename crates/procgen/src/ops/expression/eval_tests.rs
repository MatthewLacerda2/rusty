//! The evaluator: every function against its `f32` counterpart (and `math`'s, for
//! the functions the two share), and the numeric edges.

use super::super::field::Field;
use super::super::math::apply;
use super::*;
use crate::image_buf::Image;
use crate::image_buf::Size;
use crate::recipe::MathOp;

/// `src`'s value at the centre of a one-pixel image.
pub(super) fn value(src: &str) -> f32 {
    let p = compile(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
    let env = Env {
        inputs: &[],
        k: [Field::Const(0.0); 4],
        size: Size::new(1, 1),
    };
    paint(&[p], [0, 0, 0], None, &env).pixels()[0][0]
}

fn near(got: f32, want: f32) -> bool {
    got == want || (got - want).abs() <= 1e-6 * want.abs().max(1.0)
}

#[test]
fn every_function_matches_its_f32_counterpart() {
    let (x, y, z) = (1.7f32, -0.6f32, 0.3f32);
    let cases = [
        ("abs(-1.7)", x.abs()),
        ("floor(-0.6)", y.floor()),
        ("ceil(-0.6)", y.ceil()),
        ("round(1.7)", x.round()),
        ("round(-2.5)", -3.0),
        ("fract(-0.6)", y.rem_euclid(1.0)),
        ("sqrt(1.7)", x.sqrt()),
        ("exp(-0.6)", y.exp()),
        ("log(1.7)", x.ln()),
        ("sin(1.7)", x.sin()),
        ("cos(1.7)", x.cos()),
        ("tan(0.3)", z.tan()),
        ("atan2(1.7, -0.6)", x.atan2(y)),
        ("min(1.7, -0.6)", y),
        ("max(1.7, -0.6)", x),
        ("pow(1.7, 0.3)", x.powf(z)),
        ("mod(1.7, 0.3)", x - z * (x / z).floor()),
        ("mod(-1.7, 0.3)", -x - z * (-x / z).floor()),
        ("clamp(1.7, -0.6, 0.3)", z),
        ("clamp(-1, 0.3, -0.6)", y),
        ("mix(1.7, -0.6, 0.3)", x + (y - x) * z),
        ("step(0.3, 1.7)", 1.0),
        ("step(0.3, -0.6)", 0.0),
        ("step(0.3, 0.3)", 1.0),
        ("smoothstep(0, 1, 0.3)", z * z * (3.0 - 2.0 * z)),
        ("smoothstep(0, 1, 1.7)", 1.0),
        ("smoothstep(0.3, 0.3, 0.3)", 1.0),
        ("sign(-0.6)", -1.0),
        ("sign(1.7)", 1.0),
        ("sign(0)", 0.0),
        ("1.7 <= 1.7", 1.0),
        ("1.7 >= -0.6", 1.0),
        ("1.7 > 1.7", 0.0),
        ("1.7 != -0.6", 1.0),
    ];
    for (src, want) in cases {
        assert!(near(value(src), want), "{src}: {} != {want}", value(src));
    }
}

#[test]
fn the_functions_math_has_are_maths_own() {
    let pairs = [
        ("1.7 + -0.6", MathOp::Add),
        ("1.7 - -0.6", MathOp::Subtract),
        ("1.7 * -0.6", MathOp::Multiply),
        ("1.7 / -0.6", MathOp::Divide),
        ("1.7 ^ -0.6", MathOp::Power),
        ("min(1.7, -0.6)", MathOp::Min),
        ("max(1.7, -0.6)", MathOp::Max),
        ("abs(1.7)", MathOp::Abs),
        ("fract(1.7)", MathOp::Fract),
        ("sqrt(1.7)", MathOp::Sqrt),
        ("sin(1.7)", MathOp::Sin),
        ("cos(1.7)", MathOp::Cos),
        ("atan2(1.7, -0.6)", MathOp::Atan2),
    ];
    for (src, op) in pairs {
        let want = apply(op, 1.7, -0.6);
        assert_eq!(value(src).to_bits(), want.to_bits(), "{src}");
    }
}

#[test]
fn numeric_edges_follow_math() {
    assert_eq!(value("1 / 0"), 0.0, "divide's zero");
    assert_eq!(value("sqrt(-4)"), 0.0, "a negative reads as 0");
    assert_eq!(value("(-8) ^ 0.5"), 0.0);
    assert_eq!(value("log(0)"), 0.0);
    assert_eq!(value("log(-1)"), 0.0);
    assert_eq!(value("mod(3, 0)"), 0.0);
    // inf − inf is NaN; `math`'s sqrt reads it as 0 (`NaN.max(0)`), as does `^`.
    assert!(value("exp(100) - exp(100)").is_nan());
    assert_eq!(value("sqrt(exp(100) - exp(100))"), 0.0);
    assert!(
        value("clamp(exp(100) - exp(100), 0, 1)").is_nan(),
        "clamp keeps NaN"
    );
    let nan_bound = value("clamp(2, exp(100) - exp(100), 1)");
    assert_eq!(nan_bound, 1.0, "a NaN bound is no bound, never a panic");
    assert_eq!(value("exp(100) - exp(100) < 1"), 0.0, "NaN compares false");
}

#[test]
fn inputs_channels_coordinates_and_constants_are_read_per_pixel() {
    let img = Image::fill_uv(Size::new(4, 2), |u, v| [u, v, u * v, 0.25]);
    let inputs = [&img];
    let k = [0.5, 2.0, 0.0, -1.0].map(Field::Const);
    let env = Env {
        inputs: &inputs,
        k,
        size: Size::new(4, 2),
    };
    let progs = ["a - u", "a.g - v", "a.b - u * v", "a.a * k2 - k1"].map(|s| compile(s).unwrap());
    let out = paint(&progs, [0, 1, 2], Some(3), &env);
    assert!(
        out.pixels().iter().all(|p| *p == [0.0; 4]),
        "{:?}",
        out.pixels()
    );
    let k4 = paint(&[compile("k4").unwrap()], [0, 0, 0], None, &env);
    assert_eq!(
        k4.pixels()[5],
        [-1.0, -1.0, -1.0, 1.0],
        "alpha is 1 by default"
    );
}
