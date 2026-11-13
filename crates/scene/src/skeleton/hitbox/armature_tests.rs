//! Hitboxes and a ragdoll on a Blender export (#1822): the mini humanoid's mesh
//! moved under an `Armature` turned +90° about X and scaled 0.01, its own space
//! Z-up centimetres, the world geometry unchanged. The hitboxes and the joints
//! come out as they do on the plain rig: sizes are read in model units, and the
//! ragdoll's front is the model's, not the mesh's own +Z (pointing down).

use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Quat, Vec3};

use super::rig::{rig, skin, vertices};
use super::HitboxOptions;
use crate::skeleton::RagdollOptions;
use crate::Scene;
use rusty_components::{MeshComponent, TransformComponent};

/// The mini humanoid with its mesh under `armature`: mesh-local data is the
/// plain rig's through `armature⁻¹`, so the world sees the same body.
fn exported(scene: &mut Scene, armature: Mat4) -> u32 {
    let into = armature.inverse();
    let mut skin = skin(1.0);
    skin.bind_global = skin.bind_global.iter().map(|g| into * *g).collect();
    skin.inverse_bind = skin.bind_global.iter().map(|g| g.inverse()).collect();
    skin.mesh_inverse = into;
    let mut verts = vertices();
    for v in &mut verts {
        v.position = into.transform_point3(Vec3::from(v.position)).to_array();
    }
    let parent = scene.add_entity("Armature".to_string());
    *scene.world.transform_mut(parent).unwrap() = TransformComponent::from_matrix(armature);
    let id = scene.add_entity("Rig".to_string());
    scene.set_parent(id, Some(parent)).unwrap();
    let mesh = MeshComponent {
        bind_palette: skin.bind_palette(),
        skin: Some(skin),
        ..MeshComponent::new("Asset", verts, Vec::new())
    };
    scene.world.set_mesh(id, Some(mesh));
    scene.sync_skeleton(id);
    id
}

/// Per bone, in joint order: (name, hitbox shape, joint axis and swing axis
/// in world space).
fn ragdoll_of(scene: &mut Scene, owner: u32) -> Vec<(String, String, Vec3, Vec3)> {
    scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    let made = scene
        .build_ragdoll(owner, &RagdollOptions::default())
        .unwrap();
    made.into_iter()
        .map(|(name, bone)| {
            let shape = scene
                .hitbox_of(bone)
                .and_then(|h| scene.world.collider(h).map(|c| c.shape.clone()));
            let joint = scene
                .world
                .joint(bone)
                .map(|j| j.clone())
                .unwrap_or_default();
            let w = &scene.world;
            let axis = w.transform_direction(bone, joint.axis);
            let swing = w.transform_direction(bone, joint.swing_axis);
            (name, format!("{shape:?}"), axis, swing)
        })
        .collect()
}

#[test]
fn an_exported_rig_gets_the_plain_rigs_hitboxes_and_joints() {
    let armature = Mat4::from_scale_rotation_translation(
        Vec3::splat(0.01),
        Quat::from_rotation_x(FRAC_PI_2),
        Vec3::ZERO,
    );
    let (mut a, mut b) = (Scene::new(), Scene::new());
    let plain = rig(&mut a);
    let plain = ragdoll_of(&mut a, plain);
    let export = exported(&mut b, armature);
    let export = ragdoll_of(&mut b, export);
    let names =
        |r: &[(String, String, Vec3, Vec3)]| r.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    assert_eq!(
        names(&export),
        names(&plain),
        "the finger folds in, in metres"
    );
    for (p, e) in plain.iter().zip(&export) {
        assert!(p.2.distance(e.2) < 1e-3, "{}: axis {} vs {}", p.0, e.2, p.2);
        assert!(
            p.3.distance(e.3) < 1e-3,
            "{}: swing {} vs {}",
            p.0,
            e.3,
            p.3
        );
    }
}
