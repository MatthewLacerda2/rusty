//! The ragdoll tests' character: an eleven-bone humanoid with Mixamo names
//! (`mixamorig:Hips`, `…LeftForeArm`, …), facing +Z with its left along +X, a box
//! of skinned vertices per bone, and a `Run` clip that carries the hips along +Z
//! at 2 m/s. A static floor sits under it at y 0.

use glam::{Mat4, Vec3};
use rusty::asset::{AnimationClip, Interpolation, JointTrack, JointTransform, SkinData, Track};
use rusty::components::mesh::Vertex;
use rusty::components::{AnimatorComponent, ColliderComponent, ColliderShape, MeshComponent};
use rusty::scene::Scene;

/// (name, parent slot, bind position, vertex box centre, box size), mesh space.
type Bone = (&'static str, Option<usize>, [f32; 3], [f32; 3], [f32; 3]);

#[rustfmt::skip]
const BONES: [Bone; 11] = [
    ("Hips", None, [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.34, 0.2, 0.2]),
    ("Spine", Some(0), [0.0, 1.15, 0.0], [0.0, 1.35, 0.0], [0.36, 0.35, 0.2]),
    ("Head", Some(1), [0.0, 1.6, 0.0], [0.0, 1.72, 0.0], [0.2, 0.24, 0.22]),
    ("LeftArm", Some(1), [0.2, 1.45, 0.0], [0.33, 1.45, 0.0], [0.26, 0.09, 0.09]),
    ("LeftForeArm", Some(3), [0.46, 1.45, 0.0], [0.58, 1.45, 0.0], [0.24, 0.08, 0.08]),
    ("RightArm", Some(1), [-0.2, 1.45, 0.0], [-0.33, 1.45, 0.0], [0.26, 0.09, 0.09]),
    ("RightForeArm", Some(5), [-0.46, 1.45, 0.0], [-0.58, 1.45, 0.0], [0.24, 0.08, 0.08]),
    ("LeftUpLeg", Some(0), [0.1, 0.9, 0.0], [0.1, 0.7, 0.0], [0.13, 0.38, 0.13]),
    ("LeftLeg", Some(7), [0.1, 0.5, 0.0], [0.1, 0.3, 0.0], [0.11, 0.38, 0.11]),
    ("RightUpLeg", Some(0), [-0.1, 0.9, 0.0], [-0.1, 0.7, 0.0], [0.13, 0.38, 0.13]),
    ("RightLeg", Some(9), [-0.1, 0.5, 0.0], [-0.1, 0.3, 0.0], [0.11, 0.38, 0.11]),
];

const HIPS: Vec3 = Vec3::new(0.0, 1.0, 0.0);

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
        names: BONES.iter().map(|b| format!("mixamorig:{}", b.0)).collect(),
    }
}

fn vertices() -> Vec<Vertex> {
    let mut out = Vec::new();
    for (slot, b) in BONES.iter().enumerate() {
        let (centre, half) = (Vec3::from(b.3), Vec3::from(b.4) * 0.5);
        for corner in 0..8 {
            let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
            let p = centre + half * Vec3::new(sign(1), sign(2), sign(4));
            let mut v = Vertex::new(p, Vec3::Y, [0.0; 2]);
            v.joint_indices = [slot as u32, b.1.unwrap_or(slot) as u32, 0, 0];
            v.joint_weights = [0.8, 0.2, 0.0, 0.0];
            out.push(v);
        }
    }
    out
}

/// `Run`: the hips slide 2 m along +Z each second.
fn run_clip() -> AnimationClip {
    let mut tracks = vec![JointTrack::default(); BONES.len()];
    tracks[0].translation = Track {
        times: vec![0.0, 1.0],
        values: vec![HIPS, HIPS + Vec3::Z * 2.0],
        interpolation: Interpolation::Linear,
    };
    let mut clip = AnimationClip {
        name: "Run".to_string(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

/// A skinned `Rig` at `at`, skeleton spawned; its Animator plays `Run` when
/// `running`.
pub fn character(scene: &mut Scene, at: Vec3, running: bool) -> u32 {
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
            clips: vec![run_clip()],
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: Default::default(),
        }),
    );
    if running {
        let mut anim = AnimatorComponent::default();
        anim.play("Run".to_string());
        scene.world.set_animator(id, Some(anim));
    }
    scene.sync_skeleton(id);
    id
}

/// A static 40 × 1 × 40 floor whose top is at y 0.
pub fn floor(scene: &mut Scene) -> u32 {
    let id = scene.add_entity("Floor".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
    scene.world.set_static(id, true);
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::new(40.0, 1.0, 40.0),
            },
            is_trigger: false,
            material: Default::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    id
}

/// `name`'s bone of `rig` (the `mixamorig:` namespace added).
pub fn bone(scene: &Scene, rig: u32, name: &str) -> u32 {
    scene.find_bone(rig, &format!("mixamorig:{name}")).unwrap()
}
