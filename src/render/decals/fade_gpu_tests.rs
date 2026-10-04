//! Where surface decals do not land (#638): a surface running along the projection
//! axis fades the decal out instead of streaking it, a material that opts out
//! keeps its own look, and a stacked `DepthOnly` camera (the viewmodel) bins none.
//! Skips with no adapter.

use glam::Vec3;

use super::gpu_fixture::{centre, close, grey, red, stage, stamp};
use crate::components::{CameraComponent, ClearFlags, MaterialAsset};
use crate::render::test_gpu::headless_or_skip;
use crate::scene::Scene;

/// The ambient-lit white floor, stamped red projected against `normal` when given.
fn floor(material: MaterialAsset, normal: Option<Vec3>) -> Scene {
    let mut scene = stage(material, 1.0);
    if let Some(normal) = normal {
        stamp(&mut scene, normal, red());
    }
    scene
}

#[test]
fn gpu_a_surface_along_the_projection_axis_is_not_streaked() {
    let Some(mut renderer) = headless_or_skip(96, 96) else {
        return;
    };
    let (bare, _) = centre(&mut renderer, &floor(grey(0.8), None), false);
    let (facing, _) = centre(&mut renderer, &floor(grey(0.8), Some(Vec3::Y)), false);
    assert!(
        facing[0] > facing[1] + 40,
        "control: the stamp shows: {facing:?}"
    );
    // A wall hit's box reaching into the floor: the floor runs along its axis.
    let (along, counters) = centre(&mut renderer, &floor(grey(0.8), Some(Vec3::Z)), false);
    assert_eq!(counters.decals_visible, 1, "{counters:?}");
    assert!(
        close(along, bare, 2),
        "streaked: {along:?} vs bare {bare:?}"
    );
}

#[test]
fn gpu_a_material_that_does_not_receive_decals_keeps_its_look() {
    let Some(mut renderer) = headless_or_skip(96, 96) else {
        return;
    };
    let shy = MaterialAsset {
        receive_decals: false,
        ..grey(0.8)
    };
    let (bare, _) = centre(&mut renderer, &floor(shy.clone(), None), false);
    let (stamped, _) = centre(&mut renderer, &floor(shy, Some(Vec3::Y)), false);
    assert!(close(stamped, bare, 2), "{stamped:?} vs {bare:?}");
}

#[test]
fn gpu_a_stacked_depth_only_camera_bins_no_decals() {
    let Some(mut renderer) = headless_or_skip(96, 96) else {
        return;
    };
    let mut scene = floor(grey(0.8), Some(Vec3::Y));
    for (name, order, flags) in [
        ("World", 0, ClearFlags::Skybox),
        ("Viewmodel", 1, ClearFlags::DepthOnly),
    ] {
        let id = scene.add_entity(name.to_string());
        let camera = CameraComponent {
            render_order: order,
            clear_flags: flags,
            ..CameraComponent::default()
        };
        scene.world.set_camera(id, Some(camera));
    }
    let (_, counters) = centre(&mut renderer, &scene, true);
    assert_eq!(
        counters.decals_visible, 1,
        "only the world camera: {counters:?}"
    );
}
