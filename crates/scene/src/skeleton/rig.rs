//! A synthetic skinned rig with no file behind it: a joint chain whose root sits
//! under a rotated, scaled `Armature`, the offset a Blender export puts between
//! the mesh and the skeleton. A test hook: compiled in every build, since the
//! physics and render tests that build on it are other crates'.

use glam::{Mat4, Quat, Vec3};

use crate::Scene;
use rusty_asset::{JointTransform, SkinData};
use rusty_components::MeshComponent;

/// A three-joint chain (`hips → spine → head`, 1 unit apart on +Y) whose root
/// hangs under an `Armature` rotated 90° about X and scaled 0.5 — the offset a
/// Blender export puts between the mesh and the skeleton.
pub fn armature_skin() -> SkinData {
    chain_skin(&["hips", "spine", "head"])
}

/// A chain of joints named `names`, each 1 unit up its parent, the root under
/// [`armature_skin`]'s `Armature`: a rig to re-import an armature rig to.
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

/// An entity carrying a skinned mesh of [`armature_skin`] (no file behind it).
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
