//! A zero's sign out of every function that compares or picks: the same bits on
//! every platform, where `f32::min` / `max` would leave it to the CPU.

use super::eval_tests::value;

const NEG: u32 = 0x8000_0000;

fn bits(src: &str) -> u32 {
    value(src).to_bits()
}

#[test]
fn a_negative_zero_keeps_its_sign_wherever_a_function_passes_it_through() {
    // `0 * -1` is -0 (a `multiply` of 0 by a negative).
    assert_eq!(bits("0 * -1"), NEG);
    assert_eq!(bits("clamp(0 * -1, 0, 1)"), NEG, "inside the bounds: kept");
    assert_eq!(bits("clamp(0 * -1, 1, 0)"), NEG, "swapped bounds alike");
    assert_eq!(bits("sign(0 * -1)"), NEG);
    assert_eq!(
        bits("smoothstep(0 * -1, 1, 0 * -1)"),
        0,
        "t = -0 · … reads +0"
    );
}

#[test]
fn a_pick_between_two_zeros_is_fixed_whichever_comes_first() {
    for (min, max) in [
        ("min(0 * -1, 0)", "max(0 * -1, 0)"),
        ("min(0, 0 * -1)", "max(0, 0 * -1)"),
    ] {
        assert_eq!(bits(min), NEG, "{min}: -0 is the lesser");
        assert_eq!(bits(max), 0, "{max}: +0 is the greater");
    }
    assert_eq!(bits("abs(0 * -1)"), 0);
    assert_eq!(bits("step(0, 0 * -1)"), 1f32.to_bits(), "-0 >= 0");
    assert_eq!(bits("mod(0 * -1, 1)"), 0);
}

#[test]
fn a_negative_zero_base_reads_as_positive_zero() {
    assert_eq!(
        bits("(0 * -1) ^ -1"),
        f32::INFINITY.to_bits(),
        "+0 ^ -1, never -inf"
    );
    assert_eq!(bits("(0 * -1) ^ 3"), 0);
    assert_eq!(bits("sqrt(0 * -1)"), 0);
}
