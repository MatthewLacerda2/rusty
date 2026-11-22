//! `normal_blend` and `combine_rgb` against hand-built normals, and the upper-hemisphere
//! floor a blend never tips under.

use super::*;
use crate::hash;

const FLAT: Rgba = [0.5, 0.5, 1.0, 1.0];

fn normals(seed: u64) -> Image {
    bumpy(seed, 0.2)
}

fn bumpy(seed: u64, strength: f32) -> Image {
    let h = Image::fill_uv(16, |u, v| {
        let x = hash::unit2((u * 16.0) as i64, (v * 16.0) as i64, 0, seed);
        [x, x, x, 1.0]
    });
    bump_to_normal(&h, Field::Const(strength))
}

fn assert_close(a: &Image, b: &Image) {
    for (p, q) in a.pixels().iter().zip(b.pixels()) {
        assert!((0..4).all(|c| (p[c] - q[c]).abs() < 1e-5), "{p:?} vs {q:?}");
    }
}

#[test]
fn a_fourth_input_is_alpha_and_alpha_is_1_without_it() {
    let gray = |v| Image::filled(2, [v, 0.9, 0.9, 0.3]);
    let (r, g, b, a) = (gray(0.1), gray(0.2), gray(0.3), gray(0.4));
    let out = combine_rgb(&[&r, &g, &b, &a], 2.into());
    assert_eq!(out.pixels()[3], [0.1, 0.2, 0.3, 0.4]);
    let out = combine_rgb(&[&r, &g], 2.into());
    assert_eq!(out.pixels()[0], [0.1, 0.2, 0.0, 1.0]);
}

#[test]
fn a_flat_normal_on_either_side_returns_the_other() {
    let (n, flat) = (normals(3), Image::filled(16, FLAT));
    assert_close(&normal_blend(&n, &flat, Field::Const(1.0)), &n);
    assert_close(&normal_blend(&flat, &n, Field::Const(1.0)), &n);
}

#[test]
fn zero_strength_keeps_the_base() {
    let (base, detail) = (normals(3), normals(4));
    assert_close(&normal_blend(&base, &detail, Field::Const(0.0)), &base);
}

#[test]
fn the_blend_is_unit_length_and_tilts_further_than_the_base() {
    let tilt = |p: Rgba| ((p[0] - 0.5).powi(2) + (p[1] - 0.5).powi(2)).sqrt();
    // Two copies of one tilt add up: RNM rotates the base by the detail's tilt.
    let n = normals(5);
    let out = normal_blend(&n, &n, Field::Const(1.0));
    for (p, q) in out.pixels().iter().zip(n.pixels()) {
        assert!((decode(*p).length() - 1.0).abs() < 1e-5);
        if tilt(*q) > 0.05 {
            assert!(tilt(*p) > tilt(*q), "{p:?} vs {q:?}");
        }
    }
}

fn blue(z: f32) -> f32 {
    z * 0.5 + 0.5
}

/// The issue's café weave: a steep sine weave under a fine fibre, both tilting the
/// same way at the yarn sides, which plain RNM turned to z ≈ −0.39.
fn weave(output: &str) -> Image {
    use crate::ops::test_graph::node;
    use serde_json::json;
    let nodes = vec![
        node(
            "h",
            json!({ "op": "wave", "kind": "bands", "frequency": 16, "profile": "sine" }),
            &[],
        ),
        node(
            "n1",
            json!({ "op": "bump_to_normal", "strength": 0.1 }),
            &["h"],
        ),
        node(
            "h2",
            json!({ "op": "noise", "kind": "fbm", "scale": 64, "octaves": 3, "seed": 47 }),
            &[],
        ),
        node(
            "n2",
            json!({ "op": "bump_to_normal", "strength": 0.02 }),
            &["h2"],
        ),
        node(
            "nb",
            json!({ "op": "normal_blend", "strength": 1 }),
            &["n1", "n2"],
        ),
    ];
    let recipe = crate::recipe::TextureRecipe::new(256, nodes)
        .with_seed(1)
        .with_output(output);
    crate::evaluate(&recipe).unwrap()
}

#[test]
fn a_steep_base_turned_the_same_way_stays_above_the_floor_at_unit_length() {
    let (base, detail) = (weave("n1"), weave("n2"));
    // Without the floor the blend tips under the surface: the case is live.
    let raw = base.pixels().iter().zip(detail.pixels());
    assert!(raw
        .map(|(p, d)| rnm(decode(*p), decode(*d)).z)
        .any(|z| z < 0.0));
    let out = weave("nb");
    for p in out.pixels() {
        assert!(
            p[2] >= blue(MIN_NORMAL_Z) - 1e-6,
            "{p:?} is under the floor"
        );
        assert!(
            (decode(*p).length() - 1.0).abs() < 1e-5,
            "{p:?} is not unit"
        );
    }
}

#[test]
fn a_blend_that_stays_above_the_floor_is_rnms_own_bit_for_bit() {
    let (base, detail) = (bumpy(3, 0.05), bumpy(4, 0.05));
    let out = normal_blend(&base, &detail, Field::Const(1.0));
    for ((o, p), d) in out.pixels().iter().zip(base.pixels()).zip(detail.pixels()) {
        let n = rnm(decode(*p), decode(*d));
        assert!(n.z >= MIN_NORMAL_Z, "the pair must stay above the floor");
        let want = [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, p[3]];
        assert_eq!(o.map(f32::to_bits), want.map(f32::to_bits));
    }
}

#[test]
fn a_lifted_normal_keeps_its_lean_and_one_straight_down_reads_flat() {
    let n = upper_hemisphere(Vec3::new(0.6, -0.8, -0.0).normalize());
    assert!((n.length() - 1.0).abs() < 1e-6 && n.z == MIN_NORMAL_Z);
    assert!((n.y / n.x + 0.8 / 0.6).abs() < 1e-5, "{n:?} turned");
    assert_eq!(upper_hemisphere(Vec3::NEG_Z), Vec3::Z);
    let up = Vec3::new(0.1, 0.2, 0.9).normalize();
    assert_eq!(upper_hemisphere(up), up);
}

#[test]
fn a_base_already_under_the_floor_is_lifted_with_a_flat_detail_or_no_strength() {
    // A steep base (z ≈ 0.078, as a height step through `bump_to_normal` makes).
    let steep = Vec3::new(0.6, -0.79, 0.078).normalize();
    let pack = |n: Vec3| [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0];
    let base = Image::filled(4, pack(steep));
    let blends = [
        normal_blend(&base, &Image::filled(4, FLAT), Field::Const(1.0)),
        normal_blend(&base, &normals(4), Field::Const(0.0)),
    ];
    for out in &blends {
        for p in out.pixels() {
            let n = decode(*p);
            assert!(
                (n.z - MIN_NORMAL_Z).abs() < 1e-5,
                "{n:?} is not at the floor"
            );
            assert!((n.length() - 1.0).abs() < 1e-5, "{n:?} is not unit");
            assert!((n.y / n.x - steep.y / steep.x).abs() < 1e-4, "{n:?} turned");
        }
    }
}
