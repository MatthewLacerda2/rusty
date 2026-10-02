//! The hitbox tests' character: a five-bone mini humanoid (`pelvis → spine →
//! {head, arm → finger}`) with a box of skinned vertices per bone, and a `Swing`
//! clip that slides the arm along +Z at 6 m/s.

use glam::{Mat4, Vec3};
use rusty::asset::{AnimationClip, Interpolation, JointTrack, JointTransform, SkinData, Track};
use rusty::components::mesh::Vertex;
use rusty::components::{AnimatorComponent, MeshComponent};
use rusty::scene::skeleton::HitboxOptions;
use rusty::scene::Scene;

/// (name, parent slot, bind position, vertex box centre, box size), mesh space.
type Bone = (&'static str, Option<usize>, [f32; 3], [f32; 3], [f32; 3]);

const BONES: [Bone; 5] = [
    (
        "pelvis",
        None,
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.36, 0.2, 0.12],
    ),
    (
        "spine",
        Some(0),
        [0.0, 1.2, 0.0],
        [0.0, 1.35, 0.0],
        [0.4, 0.4, 0.2],
    ),
    (
        "head",
        Some(1),
        [0.0, 1.7, 0.0],
        [0.0, 1.85, 0.0],
        [0.2, 0.26, 0.22],
    ),
    (
        "arm",
        Some(1),
        [0.25, 1.5, 0.0],
        [0.42, 1.5, 0.0],
        [0.34, 0.1, 0.1],
    ),
    (
        "finger",
        Some(3),
        [0.6, 1.5, 0.0],
        [0.62, 1.5, 0.0],
        [0.04, 0.02, 0.02],
    ),
];

/// The arm bone's local bind translation, under the spine.
pub const ARM_LOCAL: Vec3 = Vec3::new(0.25, 0.3, 0.0);

fn skin() -> SkinData {
    let at = |slot: usize| Vec3::from(BONES[slot].2);
    let bind_global: Vec<Mat4> = (0..BONES.len())
        .map(|s| Mat4::from_translation(at(s)))
        .collect();
    let local_bind = (0..BONES.len())
        .map(|s| JointTransform {
            translation: at(s) - BONES[s].1.map_or(Vec3::ZERO, at),
            ..JointTransform::default()
        })
        .collect();
    SkinData {
        inverse_bind: bind_global.iter().map(|g| g.inverse()).collect(),
        bind_global,
        local_bind,
        parents: BONES.iter().map(|b| b.1).collect(),
        joint_nodes: (0..BONES.len()).collect(),
        mesh_inverse: Mat4::IDENTITY,
        names: BONES.iter().map(|b| b.0.to_string()).collect(),
    }
}

fn vertices() -> Vec<Vertex> {
    let mut out = Vec::new();
    for (slot, b) in BONES.iter().enumerate() {
        let (centre, half) = (Vec3::from(b.3), Vec3::from(b.4) * 0.5);
        for corner in 0..8 {
            let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
            let mut v = Vertex::new(
                centre + half * Vec3::new(sign(1), sign(2), sign(4)),
                Vec3::Y,
                [0.0; 2],
            );
            v.joint_indices = [slot as u32, b.1.unwrap_or(slot) as u32, 0, 0];
            v.joint_weights = [0.8, 0.2, 0.0, 0.0];
            out.push(v);
        }
    }
    out
}

/// `Swing`: the arm slides from its bind spot to +6 on Z over one second.
fn swing() -> AnimationClip {
    let mut tracks = vec![JointTrack::default(); BONES.len()];
    tracks[3].translation = Track {
        times: vec![0.0, 1.0],
        values: vec![ARM_LOCAL, ARM_LOCAL + Vec3::Z * 6.0],
        interpolation: Interpolation::Linear,
    };
    let mut clip = AnimationClip {
        name: "Swing".to_string(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

/// A `Rig` at `at` with its skeleton spawned and its hitboxes generated; the
/// Animator plays `Swing` when `swinging`.
pub fn character(scene: &mut Scene, at: Vec3, swinging: bool) -> u32 {
    let skin = skin();
    let id = scene.add_entity("Rig".to_string());
    scene.world.transform_mut(id).unwrap().position = at;
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Asset".to_string(),
            asset_ref: None,
            vertices: vertices(),
            indices: Vec::new(),
            bind_palette: skin.bind_palette(),
            skin: Some(skin),
            clips: vec![swing()],
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: Default::default(),
        }),
    );
    if swinging {
        let mut anim = AnimatorComponent::default();
        anim.play("Swing".to_string());
        scene.world.set_animator(id, Some(anim));
    }
    scene
        .generate_hitboxes(id, &HitboxOptions::default())
        .expect("the rig is skinned");
    id
}
