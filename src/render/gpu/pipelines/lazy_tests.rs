//! The forward pass's optional pipelines wait for the first frame that draws them
//! (#985): an opaque game frame builds none, the editor's overlays build the line
//! pipeline, and a translucent solid builds the transparent one. Built by hand, every
//! one of them is a valid pipeline, so a frame that first needs one never fails on it.

use glam::Vec3;

use crate::components::material::RenderMode;
use crate::components::{MaterialAsset, MaterialComponent, ParticleBlend};
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, MeshComponent, Scene};

const RES: u32 = 16;

/// One box in front of the camera, shaded by a material in `mode`.
fn box_scene(mode: RenderMode) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = crate::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    let material = MaterialComponent {
        material: "m".to_string(),
    };
    scene.world.set_material(id, Some(material));
    let asset = MaterialAsset {
        render_mode: mode,
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), asset);
    scene
}

fn draw(renderer: &mut Renderer, scene: &Scene, editor_mode: bool) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 6.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, editor_mode);
}

/// Which of (transparent, line, outline) are built.
fn built(renderer: &Renderer) -> [bool; 3] {
    [
        renderer.transparent_pipeline.is_built(),
        renderer.line_pipeline.is_built(),
        renderer.outline_pipeline.is_built(),
    ]
}

#[test]
fn gpu_optional_forward_pipelines_build_on_first_draw() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    assert_eq!(built(&renderer), [false; 3], "a new renderer builds none");

    draw(&mut renderer, &box_scene(RenderMode::Opaque), false);
    assert_eq!(
        built(&renderer),
        [false; 3],
        "an opaque game frame needs none"
    );

    draw(&mut renderer, &box_scene(RenderMode::Opaque), true);
    assert!(built(&renderer)[1], "the editor's overlays draw lines");

    draw(&mut renderer, &box_scene(RenderMode::Transparent), false);
    assert!(built(&renderer)[0], "a translucent solid draws transparent");
}

#[test]
fn gpu_every_lazy_pipeline_builds() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    renderer.outline_pipeline.get();
    renderer.line_pipeline.get();
    renderer.transparent_pipeline.get();
    for blend in [ParticleBlend::Alpha, ParticleBlend::Additive] {
        renderer.particle_renderer.pipeline_for(blend);
        renderer.ribbon_renderer.pipeline_for(blend);
    }
    assert_eq!(built(&renderer), [true; 3]);
}
