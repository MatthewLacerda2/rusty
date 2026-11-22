//! A warmed scene's first frame uploads nothing: its meshes and maps are
//! made while the scene before it draws (a map read on a worker), found by the
//! extract at the swap; a retry of a resident scene makes nothing at all. A file
//! swapped in cold uploads on that frame.

use std::collections::BTreeSet;

use glam::Vec3;

use rusty::asset::MapRef;
use rusty::components::MaterialComponent;
use rusty::edit::load::prepare::PreparedScene;
use rusty::render::warm::Warming;
use rusty::render::{RenderView, Renderer, Uploads, OFFSCREEN_FORMAT};
use rusty::scene::create::{primitive_mesh_component, Primitive};
use rusty::scene::{Camera, MaterialAsset, Scene};

const W: u32 = 32;

fn level(tag: &str, props: [Primitive; 2]) -> String {
    let dir = crate::temp::dir().join("rusty_2123_warm");
    std::fs::create_dir_all(&dir).unwrap();
    let map = dir.join(format!("{tag}.png"));
    image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 90, 40, 255]))
        .save(&map)
        .unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    for (primitive, x) in props.into_iter().zip([-1.0, 1.0]) {
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
    let path = rusty::core::paths::to_slash(&dir.join(format!("{tag}.scene")));
    rusty::edit::save_to_file(&scene, &path).unwrap();
    path
}

pub(super) fn render(renderer: &mut Renderer, scene: &Scene) -> Uploads {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, W);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    renderer.frame_counters.uploads
}

pub(super) fn first_frame(
    renderer: &mut Renderer,
    scene: &mut Scene,
    prepared: PreparedScene,
) -> Uploads {
    rusty::edit::load::swap_in_prepared(scene, prepared, &BTreeSet::new()).unwrap();
    render(renderer, scene)
}

pub(super) fn prepare(path: &str) -> PreparedScene {
    PreparedScene::from_data(rusty::scene::read_scene_file(path).unwrap(), path)
}

pub(super) fn warm(renderer: &mut Renderer, prepared: &PreparedScene) -> Uploads {
    let mut warming = Warming::plan(prepared.data());
    for _ in 0..4000 {
        if !renderer.warm_step(prepared.data(), &mut warming) && !warming.is_done() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        if warming.is_done() {
            return warming.uploads;
        }
    }
    panic!("the warm-up never finished: {warming:?}");
}

#[test]
fn gpu_a_warmed_scene_uploads_nothing_on_its_first_frame() {
    let Some(mut renderer) = rusty::render::test_gpu::headless_or_skip(W, W) else {
        return;
    };
    let path = level("first", [Primitive::Box, Primitive::Sphere]);
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    render(&mut renderer, &scene);

    let cold = first_frame(&mut renderer, &mut scene, prepare(&path));
    assert!(cold.count >= 3, "two meshes and the map, cold: {cold:?}");

    // A retry: every mesh and map is resident at its revision, so nothing is made
    // and the drawing scene's buffers are never rewritten.
    let prepared = prepare(&path);
    assert_eq!(warm(&mut renderer, &prepared), Uploads::default());
    let retried = first_frame(&mut renderer, &mut scene, prepared);
    assert_eq!(retried, Uploads::default(), "a retry uploads nothing");

    let prepared = prepare(&level("next", [Primitive::Box, Primitive::Capsule]));
    let warmed = warm(&mut renderer, &prepared);
    assert_eq!(warmed.count, 2, "the capsule and the map: {warmed:?}");
    let between = render(&mut renderer, &scene);
    assert_eq!(between.count, 0, "a frame's count leaves the warm-up's out");
    let first = first_frame(&mut renderer, &mut scene, prepared);
    assert_eq!(
        first,
        Uploads::default(),
        "nothing left for the first frame"
    );
}
