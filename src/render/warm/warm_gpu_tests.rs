//! A warmed scene's first frame uploads nothing (#2123): its meshes and maps are
//! made while the scene before it draws, found by the extract at the swap. The same
//! file swapped in cold uploads its meshes on that frame. Skips with no adapter.

use std::collections::BTreeSet;

use glam::Vec3;

use super::Warming;
use crate::app::prepare_in_background;
use crate::asset::MapRef;
use crate::components::MaterialComponent;
use crate::render::stats::Uploads;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::runtime::prepare::PreparedScene;
use crate::scene::{Camera, MaterialAsset, Scene};

const W: u32 = 32;

/// `<tmp>/rusty_2123_warm/level.scene`: a box and a sphere wearing a mapped material.
fn level() -> String {
    let dir = crate::test_temp::dir().join("rusty_2123_warm");
    std::fs::create_dir_all(&dir).unwrap();
    let map = dir.join("albedo.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 90, 40, 255]))
        .save(&map)
        .unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    for (primitive, x) in [(Primitive::Box, -1.0), (Primitive::Sphere, 1.0)] {
        let id = scene.add_entity("Prop".into());
        scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.0, -4.0);
        scene
            .world
            .set_mesh(id, primitive_mesh_component(primitive));
        scene
            .world
            .set_material(id, Some(MaterialComponent::new("mapped")));
    }
    let mapped = MaterialAsset {
        base_color_map: Some(MapRef::from(map.to_string_lossy().as_ref())),
        ..MaterialAsset::default()
    };
    scene.materials.insert("mapped".into(), mapped);
    let path = crate::core::paths::to_slash(&dir.join("level.scene"));
    scene.save_to_file(&path).unwrap();
    path
}

/// Render `scene` once; what the frame uploaded.
fn render(renderer: &mut Renderer, scene: &Scene) -> Uploads {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, W);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    renderer.frame_counters.uploads
}

/// Swap `prepared` into `scene` (no survivors) and render its first frame.
fn first_frame(renderer: &mut Renderer, scene: &mut Scene, prepared: PreparedScene) -> Uploads {
    scene.swap_in_prepared(prepared, &BTreeSet::new()).unwrap();
    render(renderer, scene)
}

fn prepare(path: &str) -> PreparedScene {
    prepare_in_background(path, &Default::default()).unwrap()
}

#[test]
fn gpu_a_warmed_scene_uploads_nothing_on_its_first_frame() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, W) else {
        return;
    };
    let path = level();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    render(&mut renderer, &scene);

    let cold = first_frame(&mut renderer, &mut scene, prepare(&path));
    assert!(cold.count >= 3, "two meshes and the map, cold: {cold:?}");

    // The same file again: its primitives carry new revisions, so they are new
    // geometry to upload, which the warm-up makes while the old scene draws.
    let prepared = prepare(&path);
    let mut warming = Warming::plan(prepared.data());
    while renderer.warm_step(prepared.data(), &mut warming) {}
    assert!(warming.is_done());
    assert_eq!(warming.uploads.count, 2, "the two meshes: {warming:?}");
    let between = render(&mut renderer, &scene);
    assert_eq!(between.count, 0, "a frame's count leaves the warm-up's out");
    let warm = first_frame(&mut renderer, &mut scene, prepared);
    assert_eq!(warm, Uploads::default(), "nothing left for the first frame");
}
