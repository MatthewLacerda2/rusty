//! A bone mirrored by scale -1 (#1110), through the whole import: its rest pose
//! and its clip's key 0 are the same transform, so they must split into the same
//! parts, or blending the rest pose with the clip flattens the bone halfway (that
//! blend is the animator's test, `app::animation`'s `mirrored_tests`).

use crate::mesh_data::{JointTransform, SkinData};
use crate::{anim_data::AnimationClip, gltf};
use glam::Vec3;

/// The fixture converted, written as a `.glb` and imported: the cube's skin and
/// its one clip, as the animator receives them.
fn import() -> (SkinData, AnimationClip) {
    let fixture = super::super::fixture("mirrored_bone.fbx");
    let out = rusty_test_support::temp::dir().join("mirrored_bone.fbx.glb");
    super::super::convert_to(&fixture, &out).expect("the fixture converts");
    let mut asset = gltf::import::import(&out).expect("its glTF imports");
    let cube = asset.sub_meshes.remove(0);
    let clip = cube.clips.into_iter().next().expect("the clip imports");
    (cube.skin.expect("the cube is skinned"), clip)
}

/// The bone's first key on every path.
fn key_0(clip: &AnimationClip) -> JointTransform {
    let track = &clip.tracks[0];
    JointTransform {
        translation: track.translation.values[0],
        rotation: track.rotation.values[0],
        scale: track.scale.values[0],
    }
}

fn assert_same(a: JointTransform, b: JointTransform) {
    assert!(
        a.translation.abs_diff_eq(b.translation, 1e-4),
        "{a:?} vs {b:?}"
    );
    assert!(
        a.rotation.dot(b.rotation).abs() > 1.0 - 1e-5,
        "{a:?} vs {b:?}"
    );
    assert!(a.scale.abs_diff_eq(b.scale, 1e-5), "{a:?} vs {b:?}");
}

#[test]
fn the_rest_pose_and_key_0_decompose_identically() {
    let (skin, clip) = import();
    assert_same(skin.local_bind[0], key_0(&clip));
    // Z up: the file's -1 on Y lands on glTF's Z, sign kept, as Unity keeps it.
    let scale = skin.local_bind[0].scale;
    assert!(
        scale.abs_diff_eq(Vec3::new(1.0, 1.0, -1.0), 1e-5),
        "{scale}"
    );
}
