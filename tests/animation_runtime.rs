//! Animation runtime end-to-end (issue #80): an imported skinned mesh + a playing
//! animator re-poses into the mesh's `pose_palette`, the result differs from the
//! bind pose, and the whole path is deterministic.

use glam::{Mat4, Vec3};
use rusty::app::animation::repose_mesh;
use rusty::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use rusty::asset::mesh_data::{JointTransform, SkinData};
use rusty::components::{AnimatorComponent, MeshComponent};
use rusty::scene::DirtyFlag;

/// A one-joint skin whose joint slides +X over [0, 1] s — the smallest rig that
/// produces a non-identity pose.
fn slide_skin() -> SkinData {
    SkinData {
        inverse_bind: vec![Mat4::IDENTITY],
        bind_global: vec![Mat4::IDENTITY],
        local_bind: vec![JointTransform::default()],
        parents: vec![None],
        joint_nodes: vec![0],
        mesh_inverse: Mat4::IDENTITY,
    }
}

fn slide_clip() -> AnimationClip {
    let mut tracks = vec![JointTrack::default()];
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

fn skinned_mesh() -> MeshComponent {
    MeshComponent {
        primitive_type: "Asset".to_string(),
        asset_ref: Some("model.glb::Animated".to_string()),
        vertices: Vec::new(),
        indices: Vec::new(),
        bind_palette: vec![Mat4::IDENTITY],
        skin: Some(slide_skin()),
        clips: vec![slide_clip()],
        pose_palette: Vec::new(),
        is_dirty: DirtyFlag::new(true),
    }
}

fn playing_animator(time: f32) -> AnimatorComponent {
    AnimatorComponent {
        current_clip: "Slide".to_string(),
        time,
        speed: 1.0,
        is_playing: true,
        ..Default::default()
    }
}

#[test]
fn repose_writes_animated_palette_distinct_from_bind() {
    let mut mesh = skinned_mesh();
    let anim = playing_animator(0.5);
    repose_mesh(&anim, &mut mesh);

    // The posed palette is populated and the renderer prefers it over the bind pose.
    assert_eq!(mesh.pose_palette.len(), 1);
    assert!(std::ptr::eq(
        mesh.active_palette().as_ptr(),
        mesh.pose_palette.as_ptr()
    ));
    // Half-way through the slide the joint has translated +1 on X — not the bind.
    let posed = mesh.pose_palette[0].to_cols_array();
    assert!((posed[12] - 1.0).abs() < 1e-5, "x = {}", posed[12]);
    assert!(!mesh.pose_palette[0].abs_diff_eq(Mat4::IDENTITY, 1e-4));
}

#[test]
fn repose_with_unknown_clip_falls_back_to_bind() {
    let mut mesh = skinned_mesh();
    let mut anim = playing_animator(0.5);
    anim.current_clip = "Missing".to_string();
    repose_mesh(&anim, &mut mesh);
    // No matching clip: the pose palette is cleared, so rendering uses the bind pose.
    assert!(mesh.pose_palette.is_empty());
    assert!(std::ptr::eq(
        mesh.active_palette().as_ptr(),
        mesh.bind_palette.as_ptr()
    ));
}

#[test]
fn repose_is_deterministic() {
    let pose = |t: f32| {
        let mut mesh = skinned_mesh();
        repose_mesh(&playing_animator(t), &mut mesh);
        mesh.pose_palette[0].to_cols_array()
    };
    assert_eq!(
        pose(0.42),
        pose(0.42),
        "same time must produce identical bytes"
    );
}
