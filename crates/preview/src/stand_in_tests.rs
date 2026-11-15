//! Tests for the stand-ins' framing, light and repeats: the plane's low view and
//! raking light, and `repeats` under UV0 and a world mapping. GPU-free.

use super::*;
use crate::build_preview_scene;
use rusty_components::DetailLayer;

fn material_subject(material: MaterialAsset) -> PreviewSubject {
    PreviewSubject::Material(Box::new(material))
}

#[test]
fn the_plane_is_a_flat_square_seen_from_its_fixed_low_angle() {
    let scene = build_preview_scene(PreviewMesh::Plane, 1, &material_subject(Default::default()));
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
    let scene = build_preview_scene(PreviewMesh::Plane, 1, &material_subject(Default::default()));
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

#[test]
fn repeats_multiply_the_tiling_under_uv0() {
    let material = MaterialAsset {
        tiling: [2.0, 0.5],
        detail: Some(DetailLayer::default()),
        ..MaterialAsset::default()
    };
    let scene = build_preview_scene(PreviewMesh::Plane, 3, &material_subject(material));
    let shown = &scene.materials["preview::material"];
    assert_eq!(shown.tiling, [6.0, 1.5]);
    assert_eq!(shown.world_size, 1.0, "UV0 leaves the world size alone");
    let detail = shown.detail.as_ref().unwrap();
    assert_eq!(
        detail.tiling,
        [1.0, 1.0],
        "the detail follows the base's tiled UV"
    );
}

#[test]
fn under_a_world_mapping_repeats_times_world_size_span_the_stand_in() {
    for mapping in [UvMapping::Planar, UvMapping::Triplanar] {
        let material = MaterialAsset {
            uv_mapping: mapping,
            world_size: 2.0,
            detail: Some(DetailLayer {
                world_size: 0.5,
                ..DetailLayer::default()
            }),
            ..MaterialAsset::default()
        };
        for (mesh, repeats) in [
            (PreviewMesh::Plane, 3),
            (PreviewMesh::Cube, 1),
            (PreviewMesh::Sphere, 4),
        ] {
            let shown = repeated(material.clone(), mesh, repeats);
            let span = repeats as f32 * shown.world_size;
            assert!(
                (span - extent(mesh)).abs() < 1e-5,
                "{mapping} {mesh}: span {span}"
            );
            assert_eq!(shown.tiling, [1.0, 1.0], "the tiling is kept");
            let detail = shown.detail.unwrap().world_size;
            assert!(
                (detail - shown.world_size / 4.0).abs() < 1e-6,
                "detail keeps its ratio"
            );
        }
    }
}

#[test]
fn zero_repeats_is_one_and_a_zero_world_size_leaves_the_detail_finite() {
    let base = MaterialAsset::default();
    assert_eq!(
        repeated(base.clone(), PreviewMesh::Sphere, 0).tiling,
        base.tiling
    );
    let broken = MaterialAsset {
        uv_mapping: UvMapping::Planar,
        world_size: 0.0,
        detail: Some(DetailLayer::default()),
        ..MaterialAsset::default()
    };
    let shown = repeated(broken, PreviewMesh::Plane, 2);
    assert_eq!(shown.world_size, 2.0);
    assert!(shown.detail.unwrap().world_size.is_finite());
}
