//! The forward pass's optional pipelines wait for the first frame that draws them
//! (#985): an opaque game frame builds none, the editor's overlays build the line
//! pipeline, and a translucent solid builds the transparent one. Warmed, as a windowed
//! renderer does at startup, every one of them is a valid pipeline.

use glam::Vec3;

use crate::components::material::RenderMode;
use crate::components::{MaterialAsset, MaterialComponent, UiBlend};
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
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 6.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, editor_mode);
}

/// Which of (transparent, line, outline mask) are built.
fn built(renderer: &Renderer) -> [bool; 3] {
    [
        super::TransparentBlend::ALL
            .into_iter()
            .any(|b| renderer.transparent_pipelines.is_built(b)),
        renderer.line_pipeline.is_built(),
        renderer.outline.mask.is_built(),
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

/// What a windowed renderer does at startup: every lazy pipeline is built, world-UI
/// blend modes no test draws included, so a game never compiles one mid-frame. A
/// shipped game (no editor overlays) leaves only the line and outline ones lazy.
#[test]
fn gpu_warm_pipelines_builds_every_lazy_pipeline() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let every = renderer.lazy_pipelines(true);
    // one transparent per blend state (#1523) + the motion-vector forward and 2 sky (#1435) + the same 3 into
    // the SSR surface target too (#1526) + the 2 primed forward ones (#1499) and the
    // primed SSR one + SSR's own pass + the motion blur's
    // 3 tile passes and gather (#1436) + the depth of field's 9 (#1445) + bloom's
    // bright pass and blur (#1467) + the lens flare's (#1443; the composite adds it, #1473) + 2 particle
    // + 2 ribbon + one world canvas per blend mode + 2 mirrored shadow + the data-mip
    // one + the opaque copy (#1523) + 4 editor (line, the outline's mask, dilation and edge).
    assert_eq!(
        every.len(),
        super::TransparentBlend::ALL.len()
            + 3
            + 3
            + 2
            + 1
            + 1
            + 4
            + 9
            + 2
            + 1
            + 2
            + 2
            + UiBlend::ALL.len()
            + 2
            + 1
            + 1
            + 4
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
