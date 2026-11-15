//! Tests for the stand-ins' framing and light: the plane's low view and raking light.
//! GPU-free; `repeats` is `repeats_tests.rs`.

use super::*;
use crate::build_preview_scene;

fn material_subject(material: MaterialAsset) -> PreviewSubject {
    PreviewSubject::Material(Box::new(material))
}

#[test]
fn the_plane_is_a_flat_square_seen_from_its_fixed_low_angle() {
    let scene = build_preview_scene(
        PreviewMesh::Plane,
        None,
        &material_subject(Default::default()),
    )
    .unwrap();
    let meshes = scene.world.ids_with_mesh();
    assert_eq!(meshes.len(), 1, "one stand-in");
    let transform = scene.world.transform(meshes[0]).unwrap();
    assert_eq!(transform.position, PLANE_CENTRE);
    let edge = transform.scale.x * crate::scene::plane_primitive_edge();
    assert!((edge - PLANE_SIZE).abs() < 1e-4, "edge {edge}");
    assert_eq!(transform.scale.x, transform.scale.z, "square");

    let orbit = framing(PreviewMesh::Plane, &material_subject(Default::default()));
    let again = framing(PreviewMesh::Plane, &material_subject(Default::default()));
    assert_eq!(
        (orbit.yaw, orbit.pitch, orbit.distance),
        (again.yaw, again.pitch, again.distance)
    );
    let view = orbit.camera().forward();
    let elevation = (-view.y).asin().to_degrees();
    assert!(
        (elevation - PLANE_VIEW_ELEVATION).abs() < 1e-3,
        "view {elevation}°"
    );
}

#[test]
fn the_plane_is_lit_by_a_light_raking_across_the_view() {
    let scene = build_preview_scene(
        PreviewMesh::Plane,
        None,
        &material_subject(Default::default()),
    )
    .unwrap();
    let light = scene.world.ids_with_light()[0];
    let travel = scene.world.transform(light).unwrap().rotation * Vec3::NEG_Z;
    let elevation = (-travel.y).asin().to_degrees();
    assert!(
        (elevation - PLANE_LIGHT_ELEVATION).abs() < 1e-3,
        "light {elevation}°"
    );
    let view = framing(PreviewMesh::Plane, &material_subject(Default::default()))
        .camera()
        .forward();
    let across = travel
        .with_y(0.0)
        .normalize()
        .dot(view.with_y(0.0).normalize());
    assert!(
        across.abs() < 1e-4,
        "the light crosses the view, dot {across}"
    );
}

#[test]
fn every_other_stand_in_and_a_model_keep_the_default_framing_and_key_light() {
    let default = OrbitState::default();
    let model = PreviewSubject::Model("a.glb".to_string());
    for (mesh, subject) in [
        (PreviewMesh::Sphere, material_subject(Default::default())),
        (PreviewMesh::Suzanne, material_subject(Default::default())),
        (PreviewMesh::Plane, model.clone()),
    ] {
        let orbit = framing(mesh, &subject);
        assert_eq!(
            (orbit.yaw, orbit.pitch),
            (default.yaw, default.pitch),
            "{mesh}"
        );
    }
    let lit = |mesh| light_pose(stand_in(mesh, &model));
    assert_eq!(
        lit(PreviewMesh::Plane),
        light_pose(Some(PreviewMesh::Sphere))
    );
}
