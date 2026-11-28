use glam::{Quat, Vec3};

use crate::{readback, RenderView, OFFSCREEN_FORMAT};
use rusty_components::RenderFace;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

const RES: u32 = 32;

/// A plane facing the eye at `distance` metres, glowing `colour`: no light shades it.
fn wall(scene: &mut Scene, name: &str, distance: f32, colour: [f32; 3]) {
    let id = create_entity(scene, name, Some(Primitive::Plane));
    let mut t = scene.world.transform_mut(id).unwrap();
    t.position = Vec3::NEG_Z * distance;
    t.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    // The 15 m plane, wide enough to fill a 60° view at `distance`.
    t.scale = Vec3::splat(distance / 4.0);
    drop(t);
    let key = scene.ensure_material_key(id).unwrap();
    let m = scene.materials.get_mut(&key).unwrap();
    (m.base_color, m.emissive, m.render_face) = ([0.0; 3], colour, RenderFace::Both);
}

#[test]
fn gpu_two_walls_a_centimetre_apart_kilometres_away_do_not_fight() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    (cam.fov, cam.near, cam.far) = (60.0, 0.1, 50_000.0);
    for distance in [5_000.0, 20_000.0] {
        let mut scene = Scene::new();
        (scene.skybox_path, scene.ambient_intensity) = (String::new(), 0.0);
        // The far wall first, so a draw order that wins a tie would hide the near one.
        wall(&mut scene, "Behind", distance + 0.01, [0.0, 4.0, 0.0]);
        wall(&mut scene, "Front", distance, [4.0, 0.0, 0.0]);
        let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
        let out = view.color_target_view().unwrap();
        r.render(&mut view, &scene, &cam, &out, false);
        let target = view.color_target().unwrap();
        let px = readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES);
        let behind = px.chunks(4).filter(|p| p[1] >= p[0]).count();
        assert_eq!(behind, 0, "at {distance} m, of {} pixels", RES * RES);
    }
}
