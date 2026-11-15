//! Tests for `repeats`: under UV0 and a world mapping, left out (the material as
//! authored), and past what the renderer draws (a refusal that names the limit).
//! GPU-free.

use super::*;
use crate::build_preview_scene;
use rusty_components::DetailLayer;

fn material_subject(material: MaterialAsset) -> PreviewSubject {
    PreviewSubject::Material(Box::new(material))
}

/// A material mapped by `mapping` at `world_size`, with a detail layer at `detail`.
fn world_mapped(mapping: UvMapping, world_size: f32, detail: f32) -> MaterialAsset {
    MaterialAsset {
        uv_mapping: mapping,
        world_size,
        detail: Some(DetailLayer {
            world_size: detail,
            ..DetailLayer::default()
        }),
        ..MaterialAsset::default()
    }
}

#[test]
fn repeats_multiply_the_tiling_under_uv0() {
    let material = MaterialAsset {
        tiling: [2.0, 0.5],
        detail: Some(DetailLayer::default()),
        ..MaterialAsset::default()
    };
    let scene =
        build_preview_scene(PreviewMesh::Plane, Some(3), &material_subject(material)).unwrap();
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
        let material = world_mapped(mapping, 2.0, 0.5);
        for (mesh, repeats) in [
            (PreviewMesh::Plane, 3),
            (PreviewMesh::Cube, 1),
            (PreviewMesh::Sphere, 4),
        ] {
            let shown = repeated(material.clone(), mesh, Some(repeats)).unwrap();
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
fn zero_repeats_raise_and_a_zero_world_size_leaves_the_detail_alone() {
    let zero = repeated(MaterialAsset::default(), PreviewMesh::Sphere, Some(0));
    assert!(zero.unwrap_err().contains("1 or more"));
    let shown = repeated(
        world_mapped(UvMapping::Planar, 0.0, 1.0),
        PreviewMesh::Plane,
        Some(2),
    );
    let shown = shown.unwrap();
    assert_eq!(shown.world_size, 2.0);
    assert_eq!(shown.detail.unwrap().world_size, 1.0);
}

#[test]
fn leaving_repeats_out_shows_a_world_mapped_material_at_its_real_scale() {
    let material = world_mapped(UvMapping::Planar, 0.25, 0.1);
    for mesh in PreviewMesh::ALL {
        let scene = build_preview_scene(mesh, None, &material_subject(material.clone())).unwrap();
        let shown = &scene.materials["preview::material"];
        assert_eq!(
            format!("{shown:?}"),
            format!("{material:?}"),
            "{mesh}: as authored"
        );
    }
}

#[test]
fn a_count_past_the_renderers_tiling_range_raises_naming_what_fits() {
    let material = MaterialAsset {
        tiling: [4.0, 1.0],
        ..MaterialAsset::default()
    };
    assert!(repeated(material.clone(), PreviewMesh::Plane, Some(250)).is_ok());
    let error = repeated(material, PreviewMesh::Plane, Some(251)).unwrap_err();
    assert!(error.contains("at most 250 repeats"), "{error}");
}

#[test]
fn a_count_past_the_renderers_world_size_range_raises_naming_what_fits() {
    // 4 m / 400 = 0.01 m, the smallest world size the renderer draws.
    let material = world_mapped(UvMapping::Planar, 1.0, 1.0);
    assert!(repeated(material.clone(), PreviewMesh::Plane, Some(400)).is_ok());
    let error = repeated(material, PreviewMesh::Plane, Some(1000)).unwrap_err();
    assert!(error.contains("at most 400 repeats"), "{error}");

    // A detail layer a tenth of the base's size runs out first: 4 m × 0.1 / 40.
    let fine = world_mapped(UvMapping::Triplanar, 1.0, 0.1);
    let error = repeated(fine, PreviewMesh::Plane, Some(41)).unwrap_err();
    assert!(
        error.contains("detail") && error.contains("at most 40 repeats"),
        "{error}"
    );

    // A detail layer far coarser than the base needs enough repeats to fit: 4 × 50 / 100.
    let coarse = world_mapped(UvMapping::Planar, 1.0, 50.0);
    let error = repeated(coarse, PreviewMesh::Plane, Some(1)).unwrap_err();
    assert!(error.contains("at least 2 repeats"), "{error}");
}
