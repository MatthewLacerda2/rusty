//! Root motion of a retargeted clip (#1747): a walk borrowed by a 20 % taller
//! rig travels 20 % farther and turns as far, measured through the animator's
//! own step, as Unity's Humanoid scales it.

use glam::{Mat4, Quat};

use super::super::test_rig::animator;
use super::tests::{run, total};
use crate::asset::humanoid::rig_fixture::{rig, walk, Shape};
use crate::asset::humanoid::{retarget, BoneMap, HumanoidSettings};
use crate::asset::mesh_data::SkinData;
use crate::asset::AnimationClip;
use crate::components::{MeshComponent, Motion, RootMotion};

fn mesh(skin: SkinData, clip: AnimationClip) -> MeshComponent {
    MeshComponent {
        skin: Some(skin),
        clips: vec![clip],
        ..MeshComponent::new("Asset", Vec::new(), Vec::new())
    }
}

fn travel(mesh: &MeshComponent) -> RootMotion {
    let mut anim = animator(Motion::Clip("Walk"), None, 0.0);
    anim.apply_root_motion = true;
    total(&run(&mut anim, mesh, 70))
}

#[test]
fn a_borrowed_walk_travels_by_the_hip_height_ratio() {
    let a = rig(Shape::default());
    let b = rig(Shape {
        height: 1.2,
        axes: Quat::from_rotation_y(0.8) * Quat::from_rotation_x(0.3),
        arm_down: 0.5,
        root_offset: Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        ..Shape::default()
    });
    let none = HumanoidSettings::default();
    let (ma, mb) = (BoneMap::resolve(&a, &none), BoneMap::resolve(&b, &none));
    let moved = retarget(&walk(&a), (&a, &ma), (&b, &mb));
    let (own, borrowed) = (travel(&mesh(a.clone(), walk(&a))), travel(&mesh(b, moved)));
    assert!(own.position.length() > 0.5, "the walk travels: {own:?}");
    let off = borrowed.position - own.position * 1.2;
    assert!(off.length() < 1e-3, "{borrowed:?} vs 1.2 × {own:?}");
    assert!(
        (borrowed.yaw - own.yaw).abs() < 1e-4,
        "{borrowed:?} vs {own:?}"
    );
    assert_eq!(own.position.y, 0.0, "root motion is horizontal");
}
