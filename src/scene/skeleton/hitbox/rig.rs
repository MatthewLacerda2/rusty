//! A five-bone mini humanoid for the hitbox tests: `pelvis → spine → {head,
//! arm → finger}`, each bone with a box of skinned vertices around it.

use glam::{Mat4, Vec3};

use crate::asset::{JointTransform, SkinData};
use crate::components::mesh::Vertex;
use crate::components::MeshComponent;
use crate::scene::Scene;

/// (name, parent slot, bind position in mesh space, vertex box centre, box size).
pub(super) const BONES: [(&str, Option<usize>, [f32; 3], [f32; 3], [f32; 3]); 5] = [
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

/// The rig's skin, its bind pose scaled by `scale` (a centimetre skeleton is 0.01).
pub(super) fn skin(scale: f32) -> SkinData {
    let bind_global: Vec<Mat4> = BONES
        .iter()
        .map(|b| {
            Mat4::from_scale_rotation_translation(
                Vec3::splat(scale),
                Default::default(),
                b.2.into(),
            )
        })
        .collect();
    let local_bind = BONES
        .iter()
        .enumerate()
        .map(|(slot, b)| {
            let parent = b.1.map_or(Mat4::IDENTITY, |p| bind_global[p]);
            let (scale, rotation, translation) =
                (parent.inverse() * bind_global[slot]).to_scale_rotation_translation();
            JointTransform {
                translation,
                rotation,
                scale,
            }
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

/// Eight corner vertices per bone box, 0.8 on the bone and 0.2 on its parent.
pub(super) fn vertices() -> Vec<Vertex> {
    let mut out = Vec::new();
    for (slot, b) in BONES.iter().enumerate() {
        let (centre, half) = (Vec3::from(b.3), Vec3::from(b.4) * 0.5);
        for corner in 0..8 {
            let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
            let p = centre + half * Vec3::new(sign(1), sign(2), sign(4));
            let mut v = Vertex::new(p, Vec3::Y, [0.0, 0.0]);
            v.joint_indices = [slot as u32, b.1.unwrap_or(slot) as u32, 0, 0];
            v.joint_weights = [0.8, 0.2, 0.0, 0.0];
            out.push(v);
        }
    }
    out
}

/// A `Rig` entity carrying the skinned mini humanoid, skeleton spawned.
pub(super) fn rig(scene: &mut Scene) -> u32 {
    let skin = skin(1.0);
    let id = scene.add_entity("Rig".to_string());
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Asset".to_string(),
            asset_ref: None,
            vertices: vertices(),
            indices: Vec::new(),
            bind_palette: skin.bind_palette(),
            skin: Some(skin),
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: Default::default(),
        }),
    );
    scene.sync_skeleton(id);
    id
}
