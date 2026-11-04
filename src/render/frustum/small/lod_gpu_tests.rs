//! A static `LODGroup` with a small-object cull on one level (#1714): the whole group
//! leaves the cached shadow bake, so the level shown casts from the per-frame sweep
//! and no hidden level keeps a baked shadow. Skips when no adapter is present.

use glam::{Quat, Vec3};

use crate::components::{LodGroupComponent, LodLevel};
use crate::render::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A sun and a static two-level group 10 m down -Z, where a 60° camera at the origin
/// shows LOD1 (a 1 m group covers 0.087 of the view). `cull` gives LOD0's and
/// LOD1's max draw distance.
fn group(cull: [f32; 2]) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let at = Vec3::new(0.0, 0.0, -10.0);
    let lods = cull.map(|distance| {
        let id = create_entity(&mut scene, "Crate_LOD", Some(Primitive::Box));
        scene.world.transform_mut(id).unwrap().position = at;
        scene.world.set_static(id, true);
        scene.world.mesh_mut(id).unwrap().max_draw_distance = distance;
        id
    });
    let root = scene.world.spawn("Crate".to_string());
    scene.world.transform_mut(root).unwrap().position = at;
    scene.world.set_static(root, true);
    let levels = [0.5, 0.01]
        .into_iter()
        .zip(lods)
        .map(|(screen_height, r)| LodLevel {
            screen_height,
            renderers: vec![r],
        })
        .collect();
    let lod = LodGroupComponent { levels, size: 1.0 };
    scene.world.set_lod_group(root, Some(lod));
    scene
}

/// Two frames of `scene` on one renderer: the first bakes, the second reads the bake.
fn two_frames(scene: &Scene) -> Option<[RenderCounters; 2]> {
    let mut r: Renderer = crate::render::test_gpu::headless_or_skip(48, 32)?;
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 48, 32);
    let out = view.color_target_view().unwrap();
    let mut cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    cam.fov = 60.0;
    Some([0, 1].map(|_| {
        r.render(&mut view, scene, &cam, &out, false);
        r.frame_counters
    }))
}

#[test]
fn gpu_a_cull_on_any_level_takes_the_whole_group_out_of_the_bake() {
    // On LOD0 (a distance that culls nothing), then on LOD1.
    for cull in [[1000.0, 0.0], [0.0, 1000.0]] {
        let Some([bake, cached]) = two_frames(&group(cull)) else {
            return;
        };
        // LOD1 casts from the per-frame sweep: tested in both frames, and the bake
        // drew no LOD0 beside it (the first frame tested nothing more).
        assert!(cached.shadow_cull_tested > 0, "{cull:?}: {cached:?}");
        assert_eq!(
            bake.shadow_cull_tested, cached.shadow_cull_tested,
            "{cull:?}: a level was baked"
        );
        assert_eq!(cached.shadow_size_culled, 0, "{cached:?}");
    }
    // LOD1 culled past 5 m: no shadow left behind by a baked LOD0.
    let Some([bake, cached]) = two_frames(&group([0.0, 5.0])) else {
        return;
    };
    assert_eq!(
        bake.shadow_cull_tested, cached.shadow_cull_tested,
        "{bake:?}"
    );
    assert_eq!(
        cached.shadow_size_culled,
        cached.shadow_cull_tested - cached.shadow_frustum_culled
    );
    assert!(cached.shadow_size_culled > 0, "{cached:?}");
}
