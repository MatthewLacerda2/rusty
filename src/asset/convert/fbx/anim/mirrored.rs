//! A bone mirrored by scale -1 (#1110), through the whole import: its rest pose
//! and its clip's key 0 are the same transform, so they must split into the same
//! parts, or blending the rest pose with the clip flattens the bone halfway.

use crate::app::animation::blend_poses;
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::asset::{anim_data::AnimationClip, gltf};
use glam::{Quat, Vec3};

/// The fixture converted, written as a `.glb` and imported: the cube's skin and
/// its one clip, as the animator receives them.
fn import() -> (SkinData, AnimationClip) {
    let fixture = super::super::fixture("mirrored_bone.fbx");
    let out = crate::test_temp::dir().join("mirrored_bone.fbx.glb");
    super::super::convert_to(&fixture, &out).expect("the fixture converts");
    let mut asset = gltf::import::import(&out).expect("its glTF imports");
    let cube = asset.sub_meshes.remove(0);
    let clip = cube.clips.into_iter().next().expect("the clip imports");
    (cube.skin.expect("the cube is skinned"), clip)
}

/// The bone's first (`last = false`) or last key on every path. The paths are
/// keyed at different rates, so "last" is each path's own.
fn key(clip: &AnimationClip, last: bool) -> JointTransform {
    let track = &clip.tracks[0];
    let pick = |n: usize| if last { n - 1 } else { 0 };
    JointTransform {
        translation: track.translation.values[pick(track.translation.values.len())],
        rotation: track.rotation.values[pick(track.rotation.values.len())],
        scale: track.scale.values[pick(track.scale.values.len())],
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
    assert_same(skin.local_bind[0], key(&clip, false));
    // Z up: the file's -1 on Y lands on glTF's Z, sign kept, as Unity keeps it.
    let scale = skin.local_bind[0].scale;
    assert!(
        scale.abs_diff_eq(Vec3::new(1.0, 1.0, -1.0), 1e-5),
        "{scale}"
    );
}

/// A crossfade from a clip that leaves the bone at rest into this clip's last
/// key, halfway, through the animator's own blend: the bone keeps its mirror
/// and turns halfway, 60° about glTF's Y (the file's Z), instead of passing
/// through a zero scale.
#[test]
fn blending_the_rest_pose_with_a_later_key_does_not_flip() {
    let (skin, clip) = import();
    let last = key(&clip, true);
    let half = blend_poses(&skin.local_bind, &[last], 0.5)[0];
    assert!(
        half.scale.abs_diff_eq(Vec3::new(1.0, 1.0, -1.0), 1e-5),
        "{half:?}"
    );
    let expected = Quat::from_rotation_y(60f32.to_radians());
    assert!(half.rotation.dot(expected).abs() > 1.0 - 1e-5, "{half:?}");
    assert!(
        (half.translation - 1.5 * Vec3::Y).length() < 1e-4,
        "{half:?}"
    );
}
