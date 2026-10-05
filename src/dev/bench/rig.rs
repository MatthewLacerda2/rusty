//! The stress scene's soldier: a procedural skinned humanoid, built in code so the
//! bench needs no binary asset. Sixteen bones, each skinning one ellipsoid of
//! ~770 triangles (~12k a body, the budget of a real shooter enemy), and a looping
//! `Walk` clip that swings the limbs. Its origin is at the feet.

use glam::{Mat4, Quat, Vec3};

use crate::asset::{AnimationClip, Interpolation, JointTrack, JointTransform, SkinData, Track};
use crate::components::mesh::primitives::generate_sphere;
use crate::components::mesh::Vertex;
use crate::components::MeshComponent;

/// The mesh-cache key every soldier shares: one GPU buffer pair for all of them.
pub const PRIMITIVE: &str = "BenchSoldier";
/// The clip every soldier plays.
pub const WALK: &str = "Walk";

/// (name, parent, bind position, limb centre, limb radii), mesh space, metres.
type Bone = (&'static str, Option<usize>, [f32; 3], [f32; 3], [f32; 3]);

#[rustfmt::skip]
const BONES: [Bone; 16] = [
    ("hips",      None,     [0.0, 1.0, 0.0],    [0.0, 1.0, 0.0],    [0.18, 0.12, 0.12]),
    ("spine",     Some(0),  [0.0, 1.15, 0.0],   [0.0, 1.25, 0.0],   [0.17, 0.14, 0.11]),
    ("chest",     Some(1),  [0.0, 1.35, 0.0],   [0.0, 1.45, 0.0],   [0.21, 0.13, 0.13]),
    ("head",      Some(2),  [0.0, 1.6, 0.0],    [0.0, 1.72, 0.0],   [0.11, 0.13, 0.12]),
    ("arm_l",     Some(2),  [0.24, 1.52, 0.0],  [0.26, 1.38, 0.0],  [0.06, 0.15, 0.06]),
    ("forearm_l", Some(4),  [0.26, 1.24, 0.0],  [0.26, 1.12, 0.0],  [0.05, 0.13, 0.05]),
    ("hand_l",    Some(5),  [0.26, 0.98, 0.0],  [0.26, 0.93, 0.0],  [0.04, 0.06, 0.03]),
    ("arm_r",     Some(2),  [-0.24, 1.52, 0.0], [-0.26, 1.38, 0.0], [0.06, 0.15, 0.06]),
    ("forearm_r", Some(7),  [-0.26, 1.24, 0.0], [-0.26, 1.12, 0.0], [0.05, 0.13, 0.05]),
    ("hand_r",    Some(8),  [-0.26, 0.98, 0.0], [-0.26, 0.93, 0.0], [0.04, 0.06, 0.03]),
    ("thigh_l",   Some(0),  [0.1, 0.92, 0.0],   [0.1, 0.7, 0.0],    [0.08, 0.22, 0.08]),
    ("shin_l",    Some(10), [0.1, 0.5, 0.0],    [0.1, 0.3, 0.0],    [0.06, 0.2, 0.06]),
    ("foot_l",    Some(11), [0.1, 0.08, 0.0],   [0.1, 0.04, 0.06],  [0.05, 0.04, 0.12]),
    ("thigh_r",   Some(0),  [-0.1, 0.92, 0.0],  [-0.1, 0.7, 0.0],   [0.08, 0.22, 0.08]),
    ("shin_r",    Some(13), [-0.1, 0.5, 0.0],   [-0.1, 0.3, 0.0],   [0.06, 0.2, 0.06]),
    ("foot_r",    Some(14), [-0.1, 0.08, 0.0],  [-0.1, 0.04, 0.06], [0.05, 0.04, 0.12]),
];

/// A soldier's mesh: skinned geometry, its skin and the `Walk` clip.
pub fn soldier() -> MeshComponent {
    let skin = skin();
    let (vertices, indices) = body();
    MeshComponent {
        primitive_type: PRIMITIVE.to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: skin.bind_palette(),
        skin: Some(skin),
        clips: vec![walk()],
        pose_palette: Vec::new(),
        skeleton: Default::default(),
        is_dirty: Default::default(),
    }
}

fn at(slot: usize) -> Vec3 {
    Vec3::from(BONES[slot].2)
}

fn skin() -> SkinData {
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

/// One ellipsoid per bone, fully weighted to it.
fn body() -> (Vec<Vertex>, Vec<u32>) {
    let (unit, unit_indices) = generate_sphere(1.0, 16, 24);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (slot, bone) in BONES.iter().enumerate() {
        let (centre, radii) = (Vec3::from(bone.3), Vec3::from(bone.4));
        let base = vertices.len() as u32;
        vertices.extend(unit.iter().map(|v| {
            let mut v = *v;
            v.position = (Vec3::from(v.position) * radii + centre).to_array();
            v.normal = (Vec3::from(v.normal) / radii)
                .normalize_or_zero()
                .to_array();
            v.joint_indices = [slot as u32, 0, 0, 0];
            v.joint_weights = [1.0, 0.0, 0.0, 0.0];
            v
        }));
        indices.extend(unit_indices.iter().map(|i| base + i));
    }
    (vertices, indices)
}

/// A one-second walk cycle: legs and arms swing in opposition about X.
fn walk() -> AnimationClip {
    let mut tracks = vec![JointTrack::default(); BONES.len()];
    let swing = |amplitude: f32| Track {
        times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        values: [0.0, 1.0, 0.0, -1.0, 0.0]
            .iter()
            .map(|s| Quat::from_rotation_x(s * amplitude))
            .collect(),
        interpolation: Interpolation::Linear,
    };
    for (slot, amplitude) in [
        (10, 0.5),
        (13, -0.5),
        (11, 0.35),
        (14, -0.35),
        (4, -0.4),
        (7, 0.4),
    ] {
        tracks[slot].rotation = swing(amplitude);
    }
    let mut clip = AnimationClip {
        name: WALK.to_string(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}
