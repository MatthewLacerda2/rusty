//! The #457 sampler tests' rig: `hips` (root) with `spine` and `leg` under it,
//! `head` under `spine`, all binding at identity — plus clip and graph builders.

use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};

use crate::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use crate::asset::animation_graph::{
    AnimationGraph, GraphLayer, GraphNode, LayerBlending, ParameterDeclaration, StateMachine,
};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{AnimatorComponent, LayerState, MeshComponent, Motion, Playback};

pub const HIPS: usize = 0;
pub const SPINE: usize = 1;
pub const LEG: usize = 2;
pub const HEAD: usize = 3;

/// A skinned mesh of the rig carrying `clips`.
pub fn mesh(clips: Vec<AnimationClip>) -> MeshComponent {
    let skin = SkinData {
        inverse_bind: vec![Mat4::IDENTITY; 4],
        bind_global: vec![Mat4::IDENTITY; 4],
        local_bind: vec![JointTransform::default(); 4],
        parents: vec![None, Some(HIPS), Some(HIPS), Some(SPINE)],
        joint_nodes: vec![0, 1, 2, 3],
        mesh_inverse: Mat4::IDENTITY,
        names: ["hips", "spine", "leg", "head"].map(String::from).to_vec(),
    };
    MeshComponent {
        primitive_type: "Asset".to_string(),
        asset_ref: None,
        vertices: Vec::new(),
        indices: Vec::new(),
        bind_palette: Vec::new(),
        skin: Some(skin),
        clips,
        pose_palette: Vec::new(),
        skeleton: Default::default(),
        is_dirty: Default::default(),
    }
}

/// A clip sliding `joints` along +X from `from` to `to` over `duration` seconds.
pub fn slide(name: &str, joints: &[usize], from: f32, to: f32, duration: f32) -> AnimationClip {
    let mut tracks = vec![JointTrack::default(); 4];
    for &j in joints {
        tracks[j].translation = Track {
            times: vec![0.0, duration],
            values: vec![Vec3::X * from, Vec3::X * to],
            interpolation: Interpolation::Linear,
        };
    }
    clip(name, tracks)
}

/// A clip turning `joint` about Z from 0 to `angle` over one second.
pub fn turn(name: &str, joint: usize, angle: f32) -> AnimationClip {
    let mut tracks = vec![JointTrack::default(); 4];
    tracks[joint].rotation = Track {
        times: vec![0.0, 1.0],
        values: vec![Quat::IDENTITY, Quat::from_rotation_z(angle)],
        interpolation: Interpolation::Linear,
    };
    clip(name, tracks)
}

fn clip(name: &str, tracks: Vec<JointTrack>) -> AnimationClip {
    let mut clip = AnimationClip {
        name: name.to_string(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

pub fn node(name: &str, clip: &str) -> GraphNode {
    GraphNode {
        name: name.to_string(),
        clip: clip.to_string(),
        blend_tree: None,
        is_loop: true,
        speed: None,
    }
}

pub fn machine(nodes: Vec<GraphNode>) -> StateMachine {
    let entry = nodes[0].name.clone();
    let edges = Vec::new();
    StateMachine {
        nodes,
        edges,
        entry,
    }
}

pub fn layer(blending: LayerBlending, mask: &[&str], nodes: Vec<GraphNode>) -> GraphLayer {
    GraphLayer {
        name: "Upper".to_string(),
        weight: 1.0,
        blending,
        mask: mask.iter().map(|m| m.to_string()).collect(),
        machine: machine(nodes),
    }
}

pub fn graph(base: Vec<GraphNode>, layers: Vec<GraphLayer>) -> AnimationGraph {
    AnimationGraph {
        parameters: BTreeMap::from([("speed".to_string(), ParameterDeclaration::Float(0.0))]),
        base: machine(base),
        layers,
    }
}

/// An animator playing `base` (and `layer` on layer 1), both at `time`.
pub fn animator(base: Motion<'_>, layer: Option<Motion<'_>>, time: f32) -> AnimatorComponent {
    let playing = |motion| {
        let mut playback = Playback::default();
        playback.play(motion);
        playback.time = time;
        playback
    };
    AnimatorComponent {
        base: playing(base),
        is_playing: true,
        layers: layer
            .into_iter()
            .map(|m| LayerState {
                name: "Upper".to_string(),
                weight: 1.0,
                playback: playing(m),
            })
            .collect(),
        ..AnimatorComponent::default()
    }
}
