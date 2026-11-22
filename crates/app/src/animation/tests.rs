use super::posed::posed_palette;
use super::*;
use glam::{Mat4, Quat, Vec3};
use rusty_asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use rusty_asset::mesh_data::{JointTransform, SkinData};
use rusty_components::{Motion, Playback};

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
    let palette = posed_palette(&skin, &translate_clip(), 0.0);
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
    let palette = posed_palette(&skin, &clip, 0.5);
    let t0 = palette[0].to_cols_array();
    assert!((t0[12] - 1.0).abs() < 1e-5, "joint0 x = {}", t0[12]);

    // joint1 rides joint0's +1 on X; its inverse bind cancels its own +1 Y bind offset.
    let t1 = palette[1].to_cols_array();
    assert!((t1[12] - 1.0).abs() < 1e-5, "joint1 x = {}", t1[12]);
    assert!(t1[13].abs() < 1e-5, "joint1 y = {}", t1[13]);
}

#[test]
fn step_interpolation_holds_previous_key() {
    let skin = chain_skin();
    let mut clip = translate_clip();
    clip.tracks[0].translation.interpolation = Interpolation::Step;
    let palette = posed_palette(&skin, &clip, 0.5);
    let t0 = palette[0].to_cols_array();
    assert!(t0[12].abs() < 1e-5, "step should hold 0, got {}", t0[12]);
}

#[test]
fn rotation_slerps_between_keys() {
    let mut skin = chain_skin();
    skin.parents = vec![None, None];
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
    let palette = posed_palette(&skin, &clip, 0.5);
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

/// Lerped matrices would average +90° and -90° about Z into a zero matrix, which
/// collapses the tip onto its root.
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
        let tip = (blended[0].matrix() * blended[1].matrix())
            .w_axis
            .truncate();
        assert!(
            (tip.length() - 1.0).abs() < 1e-5,
            "t={t}: limb length {}",
            tip.length()
        );
    }
}

#[test]
fn bone_writes_cover_only_the_driven_joints() {
    let mut mesh = two_clip_mesh();
    mesh.clips = vec![translate_clip()];
    mesh.skeleton.bones = vec![Some(10), Some(11)];
    let mut anim = looping_animator("Slide");
    anim.base.time = 0.5;
    let writes = bone_writes(&anim, &mesh, None);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, 10);
    assert!(writes[0]
        .1
        .position
        .abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-5));
    mesh.skeleton.bones.clear();
    assert!(bone_writes(&anim, &mesh, None).is_empty());
}

fn named_clip(name: &str, end: f32) -> AnimationClip {
    let mut clip = translate_clip();
    clip.name = name.to_string();
    clip.tracks[0].translation.times = vec![0.0, end];
    clip.recompute_duration();
    clip
}

fn two_clip_mesh() -> MeshComponent {
    MeshComponent {
        asset_ref: Some("model.glb::Rig".to_string()),
        skin: Some(chain_skin()),
        clips: vec![named_clip("Short", 0.75), named_clip("Long", 2.5)],
        ..MeshComponent::new("Asset", Vec::new(), Vec::new())
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
    // Two lengths, so a stub returning one constant wrap fails on one of them.
    assert_eq!(wrap(&mesh.clips[..], "Short"), 0.75);
    assert_eq!(wrap(&mesh.clips[..], "Long"), 2.5);
    // Below 0, the never-wrap sentinel: no clips, or a clip the mesh lacks.
    assert!(wrap(&[][..], "Short") < 0.0);
    assert!(wrap(&mesh.clips[..], "Ghost") < 0.0);
}

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
    assert_eq!(short.base.time, 0.25);
    assert_eq!(long.base.time, 1.0);
}

#[test]
fn sampling_is_bitwise_deterministic() {
    let skin = chain_skin();
    let clip = translate_clip();
    let first = posed_palette(&skin, &clip, 0.37);
    let second = posed_palette(&skin, &clip, 0.37);
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.to_cols_array(), b.to_cols_array());
    }
}
