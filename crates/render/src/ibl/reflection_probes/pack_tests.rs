//! Tests for `pack_blend` (#1584): what the shader unpacks.

use super::*;

fn unpack(p: u32) -> (u32, u32, f32, f32) {
    let w = |bits: u32| (bits & 1023) as f32 / 1023.0;
    (p & 63, (p >> 6) & 63, w(p >> 12), w(p >> 22))
}

#[test]
fn the_sky_alone_is_zero() {
    assert_eq!(pack_blend(None), 0);
}

#[test]
fn two_probes_and_their_weights_round_trip() {
    let blend = ProbeBlend {
        first: 31,
        first_weight: 0.75,
        second: Some(4),
        second_weight: 0.25,
    };
    let (a, b, wa, wb) = unpack(pack_blend(Some(blend)));
    assert_eq!((a, b), (32, 5), "indices plus one; the last slot fits");
    assert!((wa - 0.75).abs() < 1e-3 && (wb - 0.25).abs() < 1e-3);
}

#[test]
fn one_probe_leaves_the_second_slot_empty() {
    let blend = ProbeBlend {
        first: 0,
        first_weight: 1.0,
        second: None,
        second_weight: 0.0,
    };
    assert_eq!(unpack(pack_blend(Some(blend))), (1, 0, 1.0, 0.0));
}
