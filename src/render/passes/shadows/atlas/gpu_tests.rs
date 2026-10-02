//! Point and spot shadows on the GPU (#468): a spotlight behind a wall leaves the
//! far side dark, and a point light shadows along all six cube faces. Each check
//! renders the scene with `cast_shadows` off as its control. Skips with no adapter.

use glam::{Quat, Vec2, Vec3};

use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const RES: u32 = 96;

/// A box at `position` scaled by `scale`.
fn block(scene: &mut Scene, position: Vec3, scale: Vec3) {
    let id = create_entity(scene, "Block", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
}

/// A dark scene: no sky, sun or ambient.
fn dark() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    scene
}

/// Add a `kind` light at `at`, pointing along `dir`, casting when `shadows`.
fn lamp(scene: &mut Scene, kind: Primitive, at: Vec3, dir: Vec3, shadows: bool) {
    let id = create_entity(scene, "Lamp", Some(kind));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation) = (at, Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize()));
    drop(t);
    let mut light = scene.world.light(id).unwrap().clone();
    (light.range, light.intensity, light.outer_cone) = (10.0, 30.0, 60.0);
    light.cast_shadows = shadows;
    scene.world.set_light(id, Some(light));
}

/// A camera at `eye` looking at `target`.
fn looking(eye: Vec3, target: Vec3) -> Camera {
    let d = (target - eye).normalize();
    let yaw = d.z.atan2(d.x).to_degrees();
    Camera::new(eye, yaw, d.y.asin().to_degrees())
}

/// Summed RGB of the pixels at each of `points`, as `cam` sees `scene`.
fn brightness(r: &mut Renderer, scene: &Scene, cam: &Camera, points: &[Vec3]) -> Vec<u32> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    let target = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES);
    let screen = Vec2::splat(RES as f32);
    points
        .iter()
        .map(|&p| {
            let at = cam.world_to_screen(p, screen).position;
            let (x, y) = (at.x as u32, RES - 1 - at.y as u32);
            let i = ((y * RES + x) * 4) as usize;
            px[i..i + 3].iter().map(|&c| u32::from(c)).sum()
        })
        .collect()
}

#[test]
fn gpu_a_spotlight_does_not_light_past_a_wall() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let yard = |shadows: bool| {
        let mut scene = dark();
        block(
            &mut scene,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(20.0, 1.0, 20.0),
        );
        block(
            &mut scene,
            Vec3::new(0.0, 1.5, 0.0),
            Vec3::new(0.2, 3.0, 4.0),
        );
        let at = Vec3::new(-2.0, 1.5, 0.0);
        lamp(
            &mut scene,
            Primitive::SpotLight,
            at,
            Vec3::new(1.0, -0.3, 0.0),
            shadows,
        );
        scene
    };
    let cam = Camera::new(Vec3::new(0.0, 12.0, 0.0), -90.0, -89.0);
    let points = [Vec3::new(-0.6, 0.0, 0.0), Vec3::new(2.5, 0.0, 0.0)];
    let open = brightness(&mut r, &yard(false), &cam, &points);
    let walled = brightness(&mut r, &yard(true), &cam, &points);
    let counters: RenderCounters = r.frame_counters;
    eprintln!("[spot wall] unshadowed {open:?}, shadowed {walled:?}");
    assert_eq!(
        (counters.shadowed_lights, counters.shadow_atlas_tiles),
        (1, 1)
    );
    assert!(open[1] > 60, "control: without shadows the far side is lit");
    assert!(walled[1] + 60 < open[1], "the wall shadows the far side");
    assert!(walled[0].abs_diff(open[0]) < 20, "the near side stays lit");
}

#[test]
fn gpu_a_point_light_casts_in_all_six_directions() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    for axis in axes {
        // A wall 3 m out along `axis`, a 1 m cube 2 m out, the camera off to the side.
        let side = axis.any_orthogonal_vector().normalize();
        let room = |shadows: bool| {
            let mut scene = dark();
            let wall = Vec3::splat(12.0) - axis.abs() * 11.8;
            block(&mut scene, axis * 3.0, wall);
            block(&mut scene, axis * 2.0, Vec3::ONE);
            lamp(
                &mut scene,
                Primitive::PointLight,
                Vec3::ZERO,
                Vec3::NEG_Z,
                shadows,
            );
            scene
        };
        let cam = looking(axis + side * 3.0, axis * 2.9);
        let points = [axis * 2.9, axis * 2.9 + side * 1.6];
        let open = brightness(&mut r, &room(false), &cam, &points);
        let shadowed = brightness(&mut r, &room(true), &cam, &points);
        eprintln!("[point {axis}] unshadowed {open:?}, shadowed {shadowed:?}");
        assert_eq!(r.frame_counters.shadow_atlas_tiles, 6);
        assert!(open[0] > 60, "{axis}: control, the wall is lit");
        assert!(
            shadowed[0] + 60 < open[0],
            "{axis}: the cube shadows the wall"
        );
        assert!(
            shadowed[1].abs_diff(open[1]) < 20,
            "{axis}: beside it stays lit"
        );
    }
}
