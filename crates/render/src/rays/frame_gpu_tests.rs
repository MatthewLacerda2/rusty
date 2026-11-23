use glam::Vec3;

use super::trace_gpu_tests::level;
use crate::Renderer;
use rusty_components::ShadowCasting;
use rusty_scene::Scene;

fn rebuilds(r: &mut Renderer, scene: &Scene) -> (u32, wgpu::BindGroup) {
    r.frame_counters = Default::default();
    r.extract_scene(scene);
    let tree = r.ray_tree(scene).expect("extracted");
    let group = tree.bind_group().expect("uploaded").clone();
    (r.frame_counters.ray_tree_rebuilds, group)
}

#[test]
fn gpu_a_taken_tree_stays_as_taken_until_the_frame_releases_it() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut scene = level();
    let ids = scene.world.ids_with_mesh();
    r.extract_scene(&scene);
    let taken = r.take_rays(&scene).expect("fits");
    scene.world.transform_mut(ids[3]).unwrap().position += Vec3::Y;
    let (built, held) = rebuilds(&mut r, &scene);
    assert_eq!(
        built, 0,
        "a static moved mid-frame does not rebuild a held tree"
    );
    assert!(
        held == taken,
        "every pass of the frame binds the group it took"
    );
    r.release_rays();
    let (built, next) = rebuilds(&mut r, &scene);
    assert_eq!(built, 1, "the next frame rebuilds it");
    assert!(next != taken);
}

#[test]
fn gpu_a_static_re_derived_where_it_was_keeps_the_top() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut scene = level();
    let ids = scene.world.ids_with_mesh();
    let (_, first) = rebuilds(&mut r, &scene);
    // Re-derives the object (its shadow casting) and leaves where and how it is traced.
    scene.world.mesh_mut(ids[3]).unwrap().cast_shadows = ShadowCasting::TwoSided;
    let (built, same) = rebuilds(&mut r, &scene);
    assert_eq!(built, 0, "re-derived where it was: the top is kept");
    assert!(same == first, "and its bind group");
    scene.world.transform_mut(ids[3]).unwrap().position += Vec3::Y;
    let (built, _) = rebuilds(&mut r, &scene);
    assert_eq!(built, 1, "a real move rebuilds it");
}

fn traced_level() -> Scene {
    let mut scene = level();
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut volume = rusty_scene::create::default_post_processing();
    volume.ssr.tracing = rusty_components::SsrTracing::Mixed;
    scene.world.set_post_processing(cam, Some(volume));
    scene
}

fn document(scene: &Scene) -> rusty_scene::serialize::SceneData {
    let sparse = r#"{"entities": [], "next_entity_id": 1, "selected_entity_id": null}"#;
    let mut data = rusty_scene::parse_scene_text(sparse, "rays.scene").expect("a sparse scene");
    data.entities = scene.world.collect_entities();
    data
}

#[test]
fn gpu_a_warm_up_under_mixed_builds_the_mesh_trees_before_the_first_traced_frame() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let scene = traced_level();
    let data = document(&scene);
    let mut warming = crate::warm::Warming::plan(&data);
    while r.warm_step(&data, &mut warming) {}
    r.frame_counters = Default::default();
    r.extract_scene(&scene);
    let meshes = r.ray_tree(&scene).expect("extracted").mesh_count();
    assert_eq!(meshes, 2, "the plane's and the box's trees");
    assert_eq!(
        r.frame_counters.ray_mesh_builds, 0,
        "both built in the warm-up"
    );
    let plain = document(&level());
    let mut warming = crate::warm::Warming::plan(&plain);
    while r.warm_step(&plain, &mut warming) {}
    assert!(
        r.retained.ray_meshes.is_empty(),
        "under RayMarching none is built ahead"
    );
}
