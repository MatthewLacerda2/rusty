//! A test hook: a synthetic skinned rig whose root hangs under a rotated, scaled
//! `Armature`, the offset a Blender export puts between mesh and skeleton.
use glam::{Mat4, Quat, Vec3};

use crate::Scene;
use rusty_asset::{JointTransform, SkinData};
use rusty_components::{JointComponent, MeshComponent};

pub fn armature_skin() -> SkinData {
    chain_skin(&["hips", "spine", "head"])
}

/// Joints named `names`, each 1 unit up its parent, the root under a 90° X turn and 0.5 scale.
pub fn chain_skin(names: &[&str]) -> SkinData {
    let armature = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.5),
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        Vec3::new(0.0, 0.0, 1.0),
    );
    let step = JointTransform {
        translation: Vec3::Y,
        ..JointTransform::default()
    };
    let local_bind: Vec<JointTransform> = (0..names.len())
        .map(|s| {
            if s == 0 {
                JointTransform::default()
            } else {
                step
            }
        })
        .collect();
    let parents: Vec<Option<usize>> = (0..names.len()).map(|s| s.checked_sub(1)).collect();
    let mut bind_global: Vec<Mat4> = Vec::new();
    for (slot, local) in local_bind.iter().enumerate() {
        let parent = parents[slot].map_or(armature, |p: usize| bind_global[p]);
        bind_global.push(parent * local.matrix());
    }
    SkinData {
        inverse_bind: bind_global.iter().map(|g| g.inverse()).collect(),
        bind_global,
        local_bind,
        parents,
        joint_nodes: (1..=names.len()).collect(),
        mesh_inverse: Mat4::IDENTITY,
        names: names.iter().map(|n| n.to_string()).collect(),
    }
}

pub fn armature_owner(scene: &mut Scene) -> u32 {
    let skin = armature_skin();
    let id = scene.add_entity("Rig".to_string());
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            bind_palette: skin.bind_palette(),
            skin: Some(skin),
            ..MeshComponent::new("Asset", Vec::new(), Vec::new())
        }),
    );
    id
}

pub fn reimport_to(scene: &mut Scene, rig: u32, skin: SkinData) {
    let parked = scene.take_skeleton(rig);
    let mut mesh = scene.world.mesh_mut(rig).expect("the rig has a Mesh");
    mesh.skin = Some(skin);
    drop(mesh);
    scene.restore_skeleton(rig, parked);
}

pub fn rope_to(scene: &mut Scene, owner: u32, bone: &str) -> u32 {
    let rope = scene.add_entity("Rope".to_string());
    let body = scene.find_bone(owner, bone);
    joint_to(scene, rope, body);
    rope
}

pub fn joint_to(scene: &mut Scene, id: u32, body: Option<u32>) {
    let joint = JointComponent {
        connected_body: body,
        ..JointComponent::default()
    };
    scene.world.set_joint(id, Some(joint));
}

pub fn connected(scene: &Scene, id: u32) -> Option<u32> {
    let joint = scene.world.joint(id).expect("the entity has a Joint");
    joint.connected_body
}
