//! Unit tests for the keyframe sampler (#80): known-time poses, fallback to bind,
//! crossfade blending, and bitwise determinism across repeated runs.

use super::*;
use crate::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};
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
fn blend_palettes_is_linear() {
    let a = vec![Mat4::IDENTITY];
    let b = vec![Mat4::from_translation(Vec3::new(4.0, 0.0, 0.0))];
    let mid = blend_palettes(&a, &b, 0.5);
    assert!((mid[0].to_cols_array()[12] - 2.0).abs() < 1e-5);
    // Endpoints return the pure inputs.
    assert!(blend_palettes(&a, &b, 0.0)[0].abs_diff_eq(a[0], 1e-6));
    assert!(blend_palettes(&a, &b, 1.0)[0].abs_diff_eq(b[0], 1e-6));
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
        is_dirty: Default::default(),
    }
}

fn looping_animator(clip: &str) -> AnimatorComponent {
    AnimatorComponent {
        current_clip: clip.to_string(),
        is_playing: true,
        loop_clip: true,
        ..Default::default()
    }
}

/// #312 mutation audit: `current_clip_duration` must report each clip's ACTUAL
/// length — a stub returning 0.0 / 1.0 / -1.0 fails on both clips here.
#[test]
fn current_clip_duration_reports_the_actual_clip_length() {
    let mesh = two_clip_mesh();
    let short = looping_animator("Short");
    let long = looping_animator("Long");
    assert_eq!(current_clip_duration(&short, Some(&mesh)), 0.75);
    assert_eq!(current_clip_duration(&long, Some(&mesh)), 2.5);
    // The "unknown, never wrap" sentinel: no mesh, or a clip the mesh lacks.
    assert_eq!(current_clip_duration(&short, None), 0.0);
    assert_eq!(
        current_clip_duration(&looping_animator("Ghost"), Some(&mesh)),
        0.0
    );
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
            let duration = current_clip_duration(anim, Some(&mesh));
            anim.advance(0.5, duration);
        }
    }
    // 1.0 s of playback: the 0.75 s clip has wrapped (1.0 % 0.75), the 2.5 s
    // clip has not.
    assert_eq!(short.time, 0.25);
    assert_eq!(long.time, 1.0);
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
