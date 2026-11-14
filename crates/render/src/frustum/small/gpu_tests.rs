//! Small-object culling on the GPU (#1714): a prop past its max draw distance, or
//! under its min screen size, submits nothing to any pass (camera, cascades, the
//! static bake), and settings that cull nothing leave every pixel as it was. The
//! counters say what each view tested and culled. Skips when no adapter is present.

use glam::{Quat, Vec3};

use crate::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::authoring::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const W: u32 = 96;
const H: u32 = 64;
/// The props' distances down -Z from the camera at the origin.
const DEPTHS: [f32; 4] = [4.0, 8.0, 16.0, 32.0];

/// A sun over a floor, and one 1 m box per `DEPTHS` entry, each with these
/// settings (`max_draw_distance`, `min_screen_size`) and Static when `fixed`. The
/// floor sets neither.
fn props(settings: (f32, f32), fixed: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(floor).unwrap().position = Vec3::new(0.0, -1.0, -10.0);
    for (n, z) in DEPTHS.into_iter().enumerate() {
        let id = create_entity(&mut scene, &format!("Prop_{n}"), Some(Primitive::Box));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, -z);
        scene.world.set_static(id, fixed);
        let mut mesh = scene.world.mesh_mut(id).unwrap();
        (mesh.max_draw_distance, mesh.min_screen_size) = settings;
    }
    scene
}

/// Render `scene` once on a fresh renderer from the origin looking down -Z (60°
/// vertical field of view): its pixels and counters.
fn render(scene: &Scene) -> Option<(Vec<u8>, RenderCounters)> {
    let mut r: Renderer = crate::test_gpu::headless_or_skip(W, H)?;
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    let mut cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    cam.fov = 60.0;
    r.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, texture, W, H);
    Some((px, r.frame_counters))
}

#[test]
fn gpu_settings_that_cull_nothing_draw_the_same_pixels() {
    for fixed in [false, true] {
        let Some((off, base)) = render(&props((0.0, 0.0), fixed)) else {
            return;
        };
        // Every prop is nearer than 100 m and taller than 1% of the view: the static
        // ones now cast from the per-frame sweep, into the same depths.
        let (on, c) = render(&props((100.0, 0.01), fixed)).unwrap();
        assert!(
            on == off,
            "static {fixed}: a cull that culls nothing moved pixels"
        );
        assert_eq!(
            (c.size_culled_entities, c.shadow_size_culled),
            (0, 0),
            "{c:?}"
        );
        assert_eq!(c.visible_entities, base.visible_entities);
        assert_eq!(c.cull_tested, base.cull_tested, "{c:?}");
    }
}

#[test]
fn gpu_a_prop_past_its_distance_draws_in_no_pass() {
    for fixed in [false, true] {
        let Some((_, all)) = render(&props((0.0, 0.0), fixed)) else {
            return;
        };
        let (_, near) = render(&props((10.0, 0.0), fixed)).unwrap();
        // The two props past 10 m: off-screen to the camera and every shadow view.
        assert_eq!(near.size_culled_entities, 2, "static {fixed}: {near:?}");
        assert_eq!(near.visible_entities, all.visible_entities - 2);
        assert_eq!(
            near.cull_tested, all.cull_tested,
            "the frustum tests them still"
        );
        assert!(near.shadow_size_culled >= 2, "{near:?}");
        let box_tris = 12 * u64::from(near.shadow_size_culled + 2);
        assert_eq!(
            near.triangles,
            all.triangles - box_tris,
            "{all:?} vs {near:?}"
        );
    }
}

#[test]
fn gpu_min_screen_size_culls_by_unitys_relative_height() {
    let Some((_, all)) = render(&props((0.0, 0.0), false)) else {
        return;
    };
    // 1 m at d covers 1 / (2 d tan 30°) = 0.866 / d of the view: 0.108 at 8 m,
    // 0.054 at 16 m. A 0.08 floor keeps the first two props.
    let (_, small) = render(&props((0.0, 0.08), false)).unwrap();
    assert_eq!(small.size_culled_entities, 2, "{small:?}");
    assert_eq!(small.visible_entities, all.visible_entities - 2);
}
