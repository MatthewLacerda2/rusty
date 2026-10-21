//! A bone mirrored by scale -1 (#1110): its rest pose and its clip's key 0 are
//! the same transform, so they must split into the same parts, or blending
//! them flattens the bone halfway.

use crate::asset::convert::glb;
use glam::{Quat, Vec3};
use std::path::PathBuf;

type Trs = (Vec3, Quat, Vec3);

/// The fixture's bone as glTF stores it: its rest pose, and its key 0.
fn rest_and_key_0() -> (Trs, Trs) {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/mirrored_bone.fbx");
    let scene = super::super::load(&fixture).expect("the fixture loads");
    let doc = super::super::document(&scene, &crate::test_temp::dir()).expect("it converts");
    let node = &doc.nodes[0];
    let rest = (
        Vec3::from(node.translation),
        Quat::from_array(node.rotation),
        Vec3::from(node.scale),
    );
    let (mut t, mut r, mut s) = (None, None, None);
    for channel in &doc.animations[0].channels {
        assert_eq!(channel.node, 0);
        match &channel.keys {
            glb::Keys::Translation(v) => t = Some(Vec3::from(v[0])),
            glb::Keys::Rotation(v) => r = Some(Quat::from_array(v[0])),
            glb::Keys::Scale(v) => s = Some(Vec3::from(v[0])),
        }
    }
    let key = (
        t.unwrap(),
        r.expect("a rotation key"),
        s.expect("a scale key"),
    );
    (rest, key)
}

/// Translation, rotation and scale lerped, slerped and lerped, as the animator
/// blends two poses.
fn blend(a: Trs, b: Trs, w: f32) -> Trs {
    (a.0.lerp(b.0, w), a.1.slerp(b.1, w), a.2.lerp(b.2, w))
}

fn assert_same(a: Trs, b: Trs) {
    assert!(a.0.abs_diff_eq(b.0, 1e-4), "translation {} vs {}", a.0, b.0);
    assert!(
        a.1.dot(b.1).abs() > 1.0 - 1e-5,
        "rotation {} vs {}",
        a.1,
        b.1
    );
    assert!(a.2.abs_diff_eq(b.2, 1e-5), "scale {} vs {}", a.2, b.2);
}

#[test]
fn the_rest_pose_and_key_0_decompose_identically() {
    let (rest, key) = rest_and_key_0();
    assert_same(rest, key);
    // Z up: the file's -1 on Y lands on glTF's Z, sign kept, as Unity keeps it.
    assert!(
        key.2.abs_diff_eq(Vec3::new(1.0, 1.0, -1.0), 1e-5),
        "{}",
        key.2
    );
}

#[test]
fn blending_the_rest_pose_with_key_0_changes_nothing() {
    let (rest, key) = rest_and_key_0();
    assert_same(blend(rest, key, 0.5), rest);
    assert_same(blend(rest, key, 0.5), key);
}
