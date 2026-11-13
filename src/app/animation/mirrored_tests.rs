//! A bone mirrored by scale -1 (#1110), imported from FBX, then crossfaded by the
//! animator's own blend: it keeps its mirror instead of flattening halfway. The
//! import half (rest pose and key 0 decompose alike) is `rusty-asset`'s own test.

use super::blend_poses;
use crate::asset::{import_file, JointTransform};
use glam::{Quat, Vec3};

/// The mirrored-bone fixture, copied into a temp folder and imported: its rest
/// pose and its one clip's last key on every path.
fn rest_and_last_key() -> (Vec<JointTransform>, JointTransform) {
    let fixture = crate::core::project::checkout().join("tests/fixtures/models/mirrored_bone.fbx");
    let path = crate::test_temp::dir().join("mirrored_bone.fbx");
    std::fs::copy(fixture, &path).unwrap();
    let mut asset = import_file(&path).expect("the fixture imports");
    let cube = asset.sub_meshes.remove(0);
    let track = &cube.clips[0].tracks[0];
    let last = JointTransform {
        translation: *track.translation.values.last().unwrap(),
        rotation: *track.rotation.values.last().unwrap(),
        scale: *track.scale.values.last().unwrap(),
    };
    (cube.skin.expect("the cube is skinned").local_bind, last)
}

/// A crossfade from a clip that leaves the bone at rest into this clip's last
/// key, halfway: the bone keeps its mirror and turns halfway, 60° about glTF's
/// Y (the file's Z), instead of passing through a zero scale.
#[test]
fn blending_the_rest_pose_with_a_later_key_does_not_flip() {
    let (rest, last) = rest_and_last_key();
    let half = blend_poses(&rest, &[last], 0.5)[0];
    assert!(half.scale.abs_diff_eq(Vec3::new(1.0, 1.0, -1.0), 1e-5), "{half:?}");
    let expected = Quat::from_rotation_y(60f32.to_radians());
    assert!(half.rotation.dot(expected).abs() > 1.0 - 1e-5, "{half:?}");
    assert!((half.translation - 1.5 * Vec3::Y).length() < 1e-4, "{half:?}");
}
