use glam::{Quat, Vec3};

use crate::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};
use crate::Renderer;
use rusty_components::RenderMode;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

fn card(scene: &mut Scene, mirrored: bool, cutout: bool) {
    let plane = create_entity(scene, "Card", Some(Primitive::Plane));
    let sign = if mirrored { -1.0 } else { 1.0 };
    let mut t = scene.world.transform_mut(plane).unwrap();
    (t.position, t.scale) = (Vec3::Y * 3.0, Vec3::new(0.4 * sign, 1.0, 0.4));
    drop(t);
    if cutout {
        let key = scene.ensure_material_key(plane).unwrap();
        scene.materials.get_mut(&key).unwrap().render_mode = RenderMode::Cutout;
        scene.materials.get_mut(&key).unwrap().alpha_cutoff = 0.5;
    }
}

fn shade(r: &mut Renderer, mirrored: Option<bool>, spot: bool, cutout: bool) -> [f32; 2] {
    let mut scene = if spot { spot_yard() } else { sun_yard() };
    if let Some(mirrored) = mirrored {
        card(&mut scene, mirrored, cutout);
    }
    ground_halves(r, &scene)
}

pub(super) fn sun_yard() -> Scene {
    let (mut scene, slab) = slab_yard(None);
    scene.world.set_active(slab, false);
    scene
}

pub(super) fn spot_yard() -> Scene {
    let mut scene = Scene::new();
    (scene.skybox_path, scene.ambient_intensity) = (String::new(), 0.0);
    let ground = create_entity(&mut scene, "Ground", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(ground).unwrap();
    (t.position, t.scale) = (Vec3::NEG_Y * 0.5, Vec3::new(40.0, 1.0, 40.0));
    drop(t);
    let lamp = rusty_scene::create::create_live_light(&mut scene, "Lamp", Primitive::SpotLight);
    let mut t = scene.world.transform_mut(lamp).unwrap();
    (t.position, t.rotation) = (Vec3::Y * 6.0, Quat::from_rotation_x(-1.5));
    drop(t);
    let mut light = scene.world.light(lamp).unwrap().clone();
    (light.range, light.intensity, light.outer_cone) = (12.0, 60.0, 70.0);
    light.cast_shadows = true;
    scene.world.set_light(lamp, Some(light));
    scene
}

fn assert_twins(r: &mut Renderer, spot: bool, cutout: bool) {
    let open = shade(r, None, spot, cutout);
    let plain = shade(r, Some(false), spot, cutout);
    let mirrored = shade(r, Some(true), spot, cutout);
    let case = format!("spot {spot}, cutout {cutout}");
    eprintln!("[mirror shadow, {case}] open {open:?}, plain {plain:?}, mirrored {mirrored:?}");
    for half in 0..2 {
        assert!(plain[half] + 20.0 < open[half], "control: the card shades");
        assert!(
            (mirrored[half] - plain[half]).abs() < 5.0,
            "the mirrored card shades as its twin does"
        );
    }
}

#[test]
fn gpu_a_mirrored_plane_casts_the_sun_shadow_its_twin_does() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_twins(&mut r, false, false);
}

#[test]
fn gpu_a_mirrored_plane_casts_the_spot_shadow_its_twin_does() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_twins(&mut r, true, false);
}

#[test]
fn gpu_a_mirrored_cutout_plane_casts_the_shadow_its_twin_does() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_twins(&mut r, false, true);
}
