//! The velocity target on lavapipe (#1435), past one rigid body: a skinned mesh's
//! moved bone, a viewmodel camera stacked on the world's, and a scene load's cut.

use glam::{Mat4, Quat, Vec3};

use super::gpu_fixture::{cube, eye, project, render, renderer, scene, view};
use crate::components::mesh::primitives::generate_box;
use crate::components::CameraComponent;
use crate::scene::{ClearFlags, MeshComponent, Scene};

/// Two half-metre boxes in one skinned mesh at z = -5: the left one weighted to
/// joint 0, the right one to joint 1, whose bind pose is `right`. Returns its id.
fn rig(scene: &mut Scene, right: Mat4) -> u32 {
    let id = scene.add_entity("MotionRig".to_string());
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    for (joint, x) in [(0u32, -0.75f32), (1, 0.75)] {
        let (mut vs, is) = generate_box(0.5, 0.5, 0.5);
        let base = vertices.len() as u32;
        for v in &mut vs {
            v.position[0] += x;
            v.position[2] -= 5.0;
            v.joint_indices = [joint, 0, 0, 0];
            v.joint_weights = [1.0, 0.0, 0.0, 0.0];
        }
        vertices.extend(vs);
        indices.extend(is.iter().map(|i| i + base));
    }
    let mesh = MeshComponent {
        bind_palette: vec![Mat4::IDENTITY, right],
        ..MeshComponent::new("MotionRig", vertices, indices)
    };
    scene.world.set_mesh(id, Some(mesh));
    id
}

#[test]
fn gpu_a_skinned_mesh_writes_velocity_where_its_bone_moved() {
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let cam = eye(0.0);
    // One scene, posed twice: the same entity, so its history carries over.
    let (mut posed, _) = scene(true);
    let id = rig(&mut posed, Mat4::IDENTITY);
    render(&mut r, &mut view, &posed, &cam).unwrap();
    let up = Mat4::from_translation(Vec3::Y * 0.2);
    posed.world.mesh_mut(id).unwrap().bind_palette[1] = up;
    let moved = render(&mut r, &mut view, &posed, &cam).unwrap();

    let still = moved.at_ndc(project(&cam, Vec3::new(-0.75, 0.0, -4.75)));
    let face = Vec3::new(0.75, 0.2, -4.75);
    let expected = project(&cam, face) - project(&cam, face - Vec3::Y * 0.2);
    let got = moved.at_ndc(project(&cam, face));
    eprintln!("[motion] skinned: still bone {still}, moved bone {got} (expected {expected})");
    assert!(still.length() < 1e-3, "joint 0 did not move: {still}");
    assert!((got - expected).length() < 0.01, "{got} vs {expected}");
}

#[test]
fn gpu_a_viewmodel_turning_with_its_camera_writes_no_motion() {
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let (mut scene, _) = scene(true);
    cube(&mut scene, Vec3::new(2.0, 0.0, -6.0));
    // The overlay camera and its gun, stacked over the world camera.
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        ..CameraComponent::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    let gun = cube(&mut scene, Vec3::new(0.0, 0.0, -3.0));
    scene.set_parent(gun, Some(overlay)).unwrap();

    let turn = |scene: &mut Scene, degrees: f32| {
        let yaw = Quat::from_rotation_y(-degrees.to_radians());
        scene.world.transform_mut(overlay).unwrap().rotation = yaw;
        eye(degrees)
    };
    let before = turn(&mut scene, 0.0);
    render(&mut r, &mut view, &scene, &before).unwrap();
    assert_eq!(
        r.frame_counters.velocity_passes, 2,
        "one per stacked camera"
    );
    let after = turn(&mut scene, 4.0);
    let moved = render(&mut r, &mut view, &scene, &after).unwrap();

    let world = moved.at(2, 2);
    eprintln!("[motion] viewmodel: gun {}, world {world}", moved.centre());
    assert!(
        moved.centre().length() < 2e-3,
        "the gun turns with its camera"
    );
    assert!(world.length() > 0.02, "the world camera turned: {world}");
}

#[test]
fn gpu_the_first_frame_after_a_scene_load_writes_no_object_motion() {
    let Some(mut r) = renderer() else { return };
    let mut view = view(&r);
    let (mut scene, _) = scene(true);
    let id = cube(&mut scene, Vec3::new(-0.5, 0.0, -5.0));
    let cam = eye(0.0);
    render(&mut r, &mut view, &scene, &cam).unwrap();
    // The same ids, another scene: a load, not a move.
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.5, 0.0, -5.0);
    scene.renew_id();
    let loaded = render(&mut r, &mut view, &scene, &cam).unwrap();
    let at = loaded.at_ndc(project(&cam, Vec3::new(0.5, 0.0, -4.5)));
    assert!(at.length() < 1e-4, "no smear across a load: {at}");
}
