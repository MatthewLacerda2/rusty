//! Unit tests for the keyframe sampler (#80): known-time poses, fallback to bind,
//! crossfade blending, and bitwise determinism across repeated runs.

use super::*;
use crate::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{Motion, Playback};
use glam::{Mat4, Quat, Vec3};

/// A two-joint chain skeleton: joint0 a root at the origin, joint1 its child
/// translated +Y by 1, both with identity inverse-bind and an identity mesh
/// inverse, so the bind palette is the identity (matches #79's fixture intent).
fn chain_skin() -> SkinData {
    let root = JointTransform::default();
    let child = JointTransform {
        translation: Vec3::new(0.0, 1.0, 0.0),
        ..JointTransform::default()
    };
    SkinData {
        inverse_bind: vec![
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
        ],
        bind_global: vec![
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ],
        local_bind: vec![root, child],
        parents: vec![None, Some(0)],
        joint_nodes: vec![0, 1],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["root".to_string(), "tip".to_string()],
    }
}

/// A clip that translates joint0 along +X over [0, 1] s (0 → 2), leaving joint1's
/// local track empty so it inherits the parent and keeps its bind offset.
fn translate_clip() -> AnimationClip {
    let mut tracks = vec![JointTrack::default(), JointTrack::default()];
    tracks[0].translation = Track {
        times: vec![0.0, 1.0],
        values: vec![Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)],
        interpolation: Interpolation::Linear,
    };
    let mut clip = AnimationClip {
        name: "Slide".to_string(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

#[test]
fn duration_is_latest_key_time() {
    assert_eq!(translate_clip().duration, 1.0);
}

#[test]
fn sampling_at_zero_matches_bind_pose() {
    let skin = chain_skin();
    // A clip whose only track sits at the bind value should reproduce the identity
    // bind palette at t = 0.
    let palette = sample_palette(&skin, &translate_clip(), 0.0);
    let bind = skin.bind_palette();
    assert_eq!(palette.len(), 2);
    for (got, want) in palette.iter().zip(bind.iter()) {
        assert!(got.abs_diff_eq(*want, 1e-5), "got {got:?} want {want:?}");
    }
}

#[test]
fn linear_interpolation_at_known_time() {
    let skin = chain_skin();
    let clip = translate_clip();
    // Half-way through, joint0 has slid +1 on X. inverse_bind[0] is identity, so the
    // joint0 palette column 3 (translation) is exactly (1, 0, 0).
    let palette = sample_palette(&skin, &clip, 0.5);
    let t0 = palette[0].to_cols_array();
    assert!((t0[12] - 1.0).abs() < 1e-5, "joint0 x = {}", t0[12]);

    // joint1 inherits joint0's +X slide AND keeps its own +Y bind offset; its
    // inverse_bind subtracts the bind +Y, leaving net (+1, 0, 0).
    let t1 = palette[1].to_cols_array();
    assert!((t1[12] - 1.0).abs() < 1e-5, "joint1 x = {}", t1[12]);
    assert!(t1[13].abs() < 1e-5, "joint1 y = {}", t1[13]);
}

#[test]
fn step_interpolation_holds_previous_key() {
    let skin = chain_skin();
    let mut clip = translate_clip();
    clip.tracks[0].translation.interpolation = Interpolation::Step;
    // At t = 0.5 a STEP track holds the t = 0 key (no slide yet).
    let palette = sample_palette(&skin, &clip, 0.5);
    let t0 = palette[0].to_cols_array();
    assert!(t0[12].abs() < 1e-5, "step should hold 0, got {}", t0[12]);
}

#[test]
fn rotation_slerps_between_keys() {
    let mut skin = chain_skin();
    skin.parents = vec![None, None]; // isolate joints for a clean rotation read
    let mut clip = translate_clip();
    clip.tracks[0] = JointTrack {
        rotation: Track {
            times: vec![0.0, 1.0],
            values: vec![
                Quat::IDENTITY,
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ],
            interpolation: Interpolation::Linear,
        },
        ..JointTrack::default()
    };
    // Half-way the rotation is 45° about Z; transforming +X yields (cos45, sin45, 0).
    let palette = sample_palette(&skin, &clip, 0.5);
    let rotated = palette[0].transform_vector3(Vec3::X);
    let c = std::f32::consts::FRAC_1_SQRT_2;
    assert!(
        rotated.abs_diff_eq(Vec3::new(c, c, 0.0), 1e-4),
        "got {rotated:?}"
    );
}

#[test]
fn blend_poses_lerps_translation_and_hits_the_endpoints() {
    let a = vec![JointTransform::default()];
    let b = vec![JointTransform {
        translation: Vec3::new(4.0, 0.0, 0.0),
        ..JointTransform::default()
    }];
    assert!((blend_poses(&a, &b, 0.5)[0].translation.x - 2.0).abs() < 1e-5);
    assert_eq!(blend_poses(&a, &b, 0.0), a);
    assert_eq!(blend_poses(&a, &b, 1.0), b);
}

/// #453 regression: crossfading between two opposite rotations must keep the
/// limb's length. Lerping the finished matrices (the old `blend_palettes`)
/// averaged +90° and -90° about Z into a degenerate matrix that collapsed the
/// child onto its parent; TRS blending slerps through the rest pose instead.
#[test]
fn crossfade_between_opposite_rotations_keeps_limb_length() {
    let skin = chain_skin();
    let pose = |angle: f32| {
        let mut locals = skin.local_bind.clone();
        locals[0].rotation = Quat::from_rotation_z(angle);
        locals
    };
    let half_pi = std::f32::consts::FRAC_PI_2;
    for t in [0.25, 0.5, 0.75] {
        let blended = blend_poses(&pose(half_pi), &pose(-half_pi), t);
        let tip = clip::compose_globals(&skin, &blended)[1].w_axis.truncate();
        assert!(
            (tip.length() - 1.0).abs() < 1e-5,
            "t={t}: limb length {}",
            tip.length()
        );
    }
}

/// The animator writes only the bones its clip drives: `translate_clip` keys
/// joint0 alone, so joint1's bone is left to whatever else poses it.
#[test]
fn bone_writes_cover_only_the_driven_joints() {
    let mut mesh = two_clip_mesh();
    mesh.clips = vec![translate_clip()];
    mesh.skeleton.bones = vec![10, 11];
    let mut anim = looping_animator("Slide");
    anim.base.time = 0.5;
    let writes = bone_writes(&anim, &mesh, None);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, 10);
    assert!(writes[0]
        .1
        .position
        .abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-5));
    // An unbound skeleton gets no writes.
    mesh.skeleton.bones.clear();
    assert!(bone_writes(&anim, &mesh, None).is_empty());
}

/// A named clip whose single track ends at `end` seconds (so `duration == end`).
fn named_clip(name: &str, end: f32) -> AnimationClip {
    let mut clip = translate_clip();
    clip.name = name.to_string();
    clip.tracks[0].translation.times = vec![0.0, end];
    clip.recompute_duration();
    clip
}

/// A mesh carrying two clips of different lengths (the geometry is irrelevant).
fn two_clip_mesh() -> MeshComponent {
    MeshComponent {
        primitive_type: "Asset".to_string(),
        asset_ref: Some("model.glb::Rig".to_string()),
        vertices: Vec::new(),
        indices: Vec::new(),
        bind_palette: Vec::new(),
        skin: Some(chain_skin()),
        clips: vec![named_clip("Short", 0.75), named_clip("Long", 2.5)],
        pose_palette: Vec::new(),
        skeleton: Default::default(),
        is_dirty: Default::default(),
    }
}

fn looping_animator(clip: &str) -> AnimatorComponent {
    AnimatorComponent {
        base: Playback {
            current_clip: clip.to_string(),
            loop_clip: true,
            ..Default::default()
        },
        is_playing: true,
        ..Default::default()
    }
}

/// #312 mutation audit: a clip's playhead must wrap at each clip's ACTUAL length
/// — a stub returning 0.0 / 1.0 / -1.0 fails on both clips here.
#[test]
fn a_clip_playhead_wraps_at_the_actual_clip_length() {
    let mesh = two_clip_mesh();
    let parameters = Default::default();
    let context = |clips| motion::MotionContext {
        clips,
        machine: None,
        parameters: &parameters,
    };
    let wrap = |clips, clip| context(clips).playhead(Motion::Clip(clip)).wrap;
    assert_eq!(wrap(&mesh.clips[..], "Short"), 0.75);
    assert_eq!(wrap(&mesh.clips[..], "Long"), 2.5);
    // The "unknown, never wrap" sentinel: no clips, or a clip the mesh lacks.
    assert_eq!(wrap(&[][..], "Short"), 0.0);
    assert_eq!(wrap(&mesh.clips[..], "Ghost"), 0.0);
}

/// Two clips of different lengths wrap the looping playhead at different, exact
/// times — pinning the (duration → wrap) path end to end.
#[test]
fn looping_playhead_wraps_at_each_clips_own_duration() {
    let mesh = two_clip_mesh();
    let mut short = looping_animator("Short");
    let mut long = looping_animator("Long");
    for anim in [&mut short, &mut long] {
        for _ in 0..2 {
            advance(anim, Some(&mesh), None, 0.5);
        }
    }
    // 1.0 s of playback: the 0.75 s clip has wrapped (1.0 % 0.75), the 2.5 s
    // clip has not.
    assert_eq!(short.base.time, 0.25);
    assert_eq!(long.base.time, 1.0);
}

#[test]
fn sampling_is_bitwise_deterministic() {
    let skin = chain_skin();
    let clip = translate_clip();
    // Same (skin, clip, time) must yield byte-identical matrices on every call —
    // the property the headless replay depends on.
    let first = sample_palette(&skin, &clip, 0.37);
    let second = sample_palette(&skin, &clip, 0.37);
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.to_cols_array(), b.to_cols_array());
    }
}
