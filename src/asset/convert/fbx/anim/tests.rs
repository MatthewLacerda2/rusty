//! The fixture's "Rise" stack (#1081) through the whole import: baked, keyed in
//! glTF's axes and units, and read back as the skin's clip.

use crate::asset::{gltf, ImportedAsset};
use glam::{Quat, Vec3};
use std::path::PathBuf;

fn import() -> ImportedAsset {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/skinned_cube.fbx");
    let out = crate::test_temp::dir().join("skinned_cube_anim.fbx.glb");
    super::super::convert(&fixture, &out).expect("the fixture converts");
    gltf::import::import(&out).expect("its glTF imports")
}

#[test]
fn the_stack_is_one_clip_a_second_long() {
    let asset = import();
    let clips = &asset.sub_meshes[0].clips;
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].name, "Rise");
    assert!(
        (clips[0].duration - 1.0).abs() < 1e-3,
        "{}",
        clips[0].duration
    );
}

#[test]
fn the_bone_rises_from_one_to_two_metres_up_y() {
    let asset = import();
    let track = &asset.sub_meshes[0].clips[0].tracks[0].translation;
    let (first, last) = (track.values[0], *track.values.last().unwrap());
    assert!((first - Vec3::Y).length() < 1e-4, "starts at {first}");
    assert!((last - 2.0 * Vec3::Y).length() < 1e-4, "ends at {last}");
    assert_eq!(track.times[0], 0.0);
}

#[test]
fn the_unkeyed_rotation_stays_at_rest() {
    let asset = import();
    let track = &asset.sub_meshes[0].clips[0].tracks[0].rotation;
    for q in &track.values {
        assert!(q.dot(Quat::IDENTITY).abs() > 1.0 - 1e-5, "rotated to {q}");
    }
}
