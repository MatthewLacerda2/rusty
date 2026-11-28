use glam::Vec3;

use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::material::RenderMode;
use rusty_components::{MaterialAsset, MaterialComponent, UiBlend};
use rusty_scene::{Camera, MeshComponent, Scene};

const RES: u32 = 16;

fn box_scene(mode: RenderMode) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    let material = MaterialComponent::new("m".to_string());
    scene.world.set_material(id, Some(material));
    let asset = MaterialAsset {
        render_mode: mode,
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), asset);
    scene
}

fn draw(renderer: &mut Renderer, scene: &Scene, editor_mode: bool) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 6.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, editor_mode);
}

fn built(renderer: &Renderer) -> [bool; 3] {
    [
        super::TransparentBlend::ALL
            .into_iter()
            .any(|b| renderer.transparent_pipelines.is_built(b)),
        renderer.line_pipeline.is_built(),
        super::Facing::ALL
            .into_iter()
            .any(|f| renderer.outline.mask.is_built(f)),
    ]
}

#[test]
fn gpu_optional_forward_pipelines_build_on_first_draw() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
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
fn gpu_warm_pipelines_builds_every_lazy_pipeline() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let every = renderer.lazy_pipelines(true);
    const F: usize = 3;
    assert_eq!(
        every.len(),
        2 * super::TransparentBlend::ALL.len() * F
            // SSR's Mixed translucent surface prepass.
            + F
            + 3 * F
            + (F + 2)
            + (F + 2)
            + 2 * F
            + F
            + 3 * (F - 1)
            + 3
            + 3
            + 5
            + 4
            + 3
            + 9
            + 4
            + 1
            + 4
            + 4
            + 4
            + UiBlend::ALL.len()
            + 2
            + 2
            + 2
            + 1
            + (4 + F + 2)
            + 2
            // SSR's accumulating resolve, its Mixed rays, their merge, the base
            // colours' resample, the translucents' rays and their bound.
            + 6
    );
    assert!(every.iter().all(|p| !p.is_built()), "headless stays lazy");

    renderer.warm_pipelines(false);
    assert!(renderer.lazy_pipelines(false).iter().all(|p| p.is_built()));
    assert_eq!(
        built(&renderer),
        [true, false, false],
        "a game warms no overlays"
    );

    renderer.warm_pipelines(true);
    assert!(renderer.lazy_pipelines(true).iter().all(|p| p.is_built()));
}
