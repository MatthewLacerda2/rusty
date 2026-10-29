//! The opaque copy is made only on a frame that shows a surface reading it (#1523),
//! and counted.

use glam::Vec3;

use crate::components::{MaterialAsset, MaterialComponent, RefractionModel, RenderMode};
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A scene holding one transparent box at `z` drawn with `glass`.
fn pane(glass: MaterialAsset, z: f32) -> Scene {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Pane", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, z);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new("glass")));
    scene.materials.insert("glass".into(), glass);
    scene
}

/// Render one frame of `scene`, looking down -z, and return how many copies it made.
fn copies(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> u32 {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    r.render(view, scene, &cam, &out, false);
    r.frame_counters.opaque_copies
}

#[test]
fn gpu_the_copy_is_made_only_for_a_visible_surface_that_reads_it() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let plain = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        ..MaterialAsset::default()
    };
    let water = MaterialAsset {
        refraction_model: RefractionModel::Planar,
        ..plain.clone()
    };
    let murky = MaterialAsset {
        transmittance_color: [0.2, 0.5, 0.6],
        ..plain.clone()
    };
    assert_eq!(
        copies(&mut r, &mut view, &pane(plain, 0.0)),
        0,
        "plain glass"
    );
    assert_eq!(
        copies(&mut r, &mut view, &pane(water.clone(), 0.0)),
        1,
        "refraction"
    );
    assert_eq!(
        copies(&mut r, &mut view, &pane(murky, 0.0)),
        1,
        "absorption"
    );
    assert_eq!(
        copies(&mut r, &mut view, &pane(water, 50.0)),
        0,
        "behind the camera"
    );
}
