//! The atlas's shadows stay attached to their casters too (#665): the spot and point
//! tiles draw through the cascades' depth pipelines, so a unit box resting on the
//! floor under a shadowing spot or point light darkens the floor right up to its
//! base. Skips with no adapter.

use glam::{Quat, Vec3};

use crate::render::passes::shadows::contact_tests::{brightness, looking, RES};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// A dark scene: the floor (top at y = 0), a unit box resting on it at the origin,
/// and a shadowing `kind` light up and to its -X side, aimed past it.
fn lamp_yard(kind: Primitive, floor: Primitive) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let mut place = |kind, position, scale| {
        let id = create_entity(&mut scene, "Shape", Some(kind));
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (position, scale);
        id
    };
    match floor {
        Primitive::Plane => place(floor, Vec3::ZERO, Vec3::ONE),
        _ => place(floor, Vec3::NEG_Y * 0.5, Vec3::new(30.0, 1.0, 30.0)),
    };
    place(Primitive::Box, Vec3::Y * 0.5, Vec3::ONE);
    let at = Vec3::new(-2.0, 2.0, 0.0);
    let dir = Vec3::new(3.0, -2.0, 0.0).normalize();
    let id = place(kind, at, Vec3::ONE);
    scene.world.transform_mut(id).unwrap().rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
    let mut light = scene.world.light(id).unwrap().clone();
    (light.range, light.intensity, light.outer_cone) = (10.0, 30.0, 60.0);
    light.cast_shadows = true;
    scene.world.set_light(id, Some(light));
    scene
}

#[test]
fn gpu_a_resting_box_shadows_the_floor_up_to_its_base_under_local_lights() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let out = [0.01, 0.02, 0.03, 0.05, 0.08, 0.12, 0.2];
    let cam = looking(Vec3::new(2.4, 1.6, 0.6), Vec3::new(0.6, 0.0, 0.0));
    let mut points: Vec<Vec3> = out.iter().map(|d| Vec3::new(0.5 + d, 0.0, 0.0)).collect();
    points.push(Vec3::new(1.2, 0.0, 1.0));
    let mut gaps = Vec::new();
    for kind in [Primitive::SpotLight, Primitive::PointLight] {
        for floor in [Primitive::Plane, Primitive::Box] {
            let mut b = brightness(&mut r, &lamp_yard(kind, floor), &cam, &points);
            let lit = b.pop().unwrap();
            eprintln!("[atlas contact] {kind:?} on {floor:?}: {b:?} vs lit {lit}");
            assert!(lit > 60, "control: the floor beside the box is lit");
            let gap = out.iter().zip(&b).filter(|(_, b)| **b * 5 >= lit);
            gaps.extend(gap.map(|(d, b)| format!("{kind:?} {floor:?} {d} m: {b}/{lit}")));
        }
    }
    assert!(gaps.is_empty(), "lit floor at the contact line: {gaps:?}");
}
