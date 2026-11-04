//! Root motion of a Blender export (#1822): the mesh sits under an `Armature`
//! turned +90° about X and scaled 0.01, so its own space is Z-up centimetres.
//! The step reads the hips in model space, so the rig travels and turns what
//! its clip does, exactly as the same rig without the armature.

use std::f32::consts::FRAC_PI_2;

use glam::Vec3;

use super::super::sample_pose;
use super::super::test_rig::animator;
use super::tests::{run, total};
use super::RootBone;
use crate::asset::humanoid::rig_fixture::{blender_armature, lunge, model_rest, rig, Shape, HIPS};
use crate::asset::mesh_data::SkinData;
use crate::components::{MeshComponent, Motion, RootMotion};

fn exported() -> SkinData {
    rig(Shape {
        armature: blender_armature(),
        ..Shape::default()
    })
}

fn lunging(skin: SkinData, travel: Vec3, turn: f32) -> MeshComponent {
    MeshComponent {
        clips: vec![lunge(&skin, travel, turn)],
        skin: Some(skin),
        ..MeshComponent::new("Asset", Vec::new(), Vec::new())
    }
}

fn travel(mesh: &MeshComponent) -> RootMotion {
    let mut anim = animator(Motion::Clip("Lunge"), None, 0.0);
    total(&run(&mut anim, mesh, 70))
}

#[test]
fn an_exported_rig_travels_its_clip_in_metres_on_the_model_axis() {
    let done = travel(&lunging(exported(), Vec3::Z * 2.0, 0.0));
    assert!(done.position.distance(Vec3::Z * 2.0) < 1e-3, "{done:?}");
    assert!(done.yaw.abs() < 1e-4, "{done:?}");
}

#[test]
fn an_exported_rig_turns_about_model_up_as_the_native_one_does() {
    let travel_of = |skin| travel(&lunging(skin, Vec3::X, FRAC_PI_2));
    let (native, export) = (travel_of(rig(Shape::default())), travel_of(exported()));
    assert!((native.yaw - FRAC_PI_2).abs() < 1e-3, "{native:?}");
    assert!(
        (export.yaw - native.yaw).abs() < 1e-4,
        "{export:?} vs {native:?}"
    );
    let off = export.position.distance(native.position);
    assert!(off < 1e-3, "{export:?} vs {native:?}");
}

#[test]
fn the_stripped_pose_holds_the_hips_on_the_model_horizontal_and_keeps_the_lift() {
    let mesh = lunging(exported(), Vec3::Z * 2.0, FRAC_PI_2);
    let mut anim = animator(Motion::Clip("Lunge"), None, 0.5);
    anim.apply_root_motion = true;
    let (skin, mut pose) = sample_pose(&anim, &mesh, None).expect("a skinned rig");
    RootBone::resolve(skin, "")
        .expect("hips")
        .strip(skin, &mut pose);
    let local = pose[HIPS].expect("keyed").matrix();
    let held = skin.mesh_to_model() * skin.root_offset(HIPS) * local;
    let rest = model_rest(skin, HIPS);
    let lift = held.w_axis.truncate() - rest.w_axis.truncate();
    assert!(lift.distance(Vec3::Y * 0.1) < 1e-4, "only the lift: {lift}");
    let facing = held.z_axis.truncate().normalize();
    let bind = rest.z_axis.truncate().normalize();
    assert!(
        facing.distance(bind) < 1e-4,
        "bind facing: {facing} vs {bind}"
    );
}
