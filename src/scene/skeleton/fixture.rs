//! Test fixtures for the skeleton module: an animated glTF on disk (the asset
//! layer's two-joint `Joint0 → Joint1` rig) and a synthetic skin whose root sits
//! under a rotated, scaled `Armature`.

use glam::{Mat4, Quat, Vec3};

use crate::asset::fixtures_anim::{animated_buffer, ANIMATED_GLTF};
use crate::asset::{JointTransform, SkinData};
use crate::components::MeshComponent;
use crate::scene::Scene;

/// Write the animated fixture under a per-test folder and return its
/// `path::Animated` reference.
pub(super) fn animated_model(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_453_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("anim.bin"), animated_buffer()).unwrap();
    let path = dir.join("model.gltf");
    std::fs::write(&path, ANIMATED_GLTF).unwrap();
    format!("{}::Animated", path.to_string_lossy().replace('\\', "/"))
}

/// Instantiate the animated fixture as `Hero`, skeleton and all.
pub(super) fn hero(scene: &mut Scene, tag: &str) -> u32 {
    crate::scene::asset_instance::instantiate_asset(
        scene,
        &animated_model(tag),
        Some("Hero"),
        Vec3::ZERO,
    )
    .expect("the fixture instantiates")
}

/// A three-joint chain (`hips → spine → head`, 1 unit apart on +Y) whose root
/// hangs under an `Armature` rotated 90° about X and scaled 0.5 — the offset a
/// Blender export puts between the mesh and the skeleton.
pub(super) fn armature_skin() -> SkinData {
    let armature = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.5),
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        Vec3::new(0.0, 0.0, 1.0),
    );
    let step = JointTransform {
        translation: Vec3::Y,
        ..JointTransform::default()
    };
    let local_bind = vec![JointTransform::default(), step, step];
    let parents = vec![None, Some(0), Some(1)];
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
        joint_nodes: vec![1, 2, 3],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["hips".into(), "spine".into(), "head".into()],
    }
}

/// An entity carrying a skinned mesh of [`armature_skin`] (no file behind it).
pub(super) fn armature_owner(scene: &mut Scene) -> u32 {
    let skin = armature_skin();
    let id = scene.add_entity("Rig".to_string());
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Asset".to_string(),
            asset_ref: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_palette: skin.bind_palette(),
            skin: Some(skin),
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: Default::default(),
        }),
    );
    id
}
