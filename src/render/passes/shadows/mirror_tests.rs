//! A mirrored caster casts the shadow its unmirrored twin does (#1134). The depth
//! pass culls back faces, and a negative world scale reverses a mesh's winding, so
//! a mirrored caster used to store its far side, or nothing: a one-sided `Plane`
//! turned inside-out by `scale.x < 0` cast no shadow at all. Checked for the sun's
//! cascades and the spot/point atlas, which share the pipelines, and for a Cutout
//! card, whose clipped caster has a mirrored pipeline of its own. Skips with no
//! adapter.

use glam::{Quat, Vec3};

use crate::components::RenderMode;
use crate::render::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};
use crate::render::Renderer;
use crate::scene::authoring::{create_entity, material, Primitive};
use crate::scene::Scene;

/// A 6 x 6 one-sided `Plane` at y = 3, mirrored along X when `mirrored`; with
/// `cutout`, a solid Cutout material, so its shadow takes the clip pipeline (#648).
fn card(scene: &mut Scene, mirrored: bool, cutout: bool) {
    let plane = create_entity(scene, "Card", Some(Primitive::Plane));
    let sign = if mirrored { -1.0 } else { 1.0 };
    let mut t = scene.world.transform_mut(plane).unwrap();
    (t.position, t.scale) = (Vec3::Y * 3.0, Vec3::new(0.4 * sign, 1.0, 0.4));
    drop(t);
    if cutout {
        let key = material::ensure_material_key(scene, plane).unwrap();
        material::set_render_mode(&mut scene.materials, &key, RenderMode::Cutout);
        material::set_alpha_cutoff(&mut scene.materials, &key, 0.5);
    }
}

/// The ground's brightness under each half of [`card`] (none for `None`), lit by
/// [`slab_yard`]'s sun or, with `spot`, by a spotlight overhead alone.
fn shade(r: &mut Renderer, mirrored: Option<bool>, spot: bool, cutout: bool) -> [f32; 2] {
    let mut scene = if spot { spot_yard() } else { sun_yard() };
    if let Some(mirrored) = mirrored {
        card(&mut scene, mirrored, cutout);
    }
    ground_halves(r, &scene)
}

/// [`slab_yard`] without its slab: ground under a high sun.
fn sun_yard() -> Scene {
    let (mut scene, slab) = slab_yard(None);
    scene.world.set_active(slab, false);
    scene
}

/// Ground in the dark, under one shadowing spotlight pointing down from y = 6.
fn spot_yard() -> Scene {
    let mut scene = Scene::new();
    (scene.skybox_path, scene.ambient_intensity) = (String::new(), 0.0);
    let ground = create_entity(&mut scene, "Ground", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(ground).unwrap();
    (t.position, t.scale) = (Vec3::NEG_Y * 0.5, Vec3::new(40.0, 1.0, 40.0));
    drop(t);
    let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::SpotLight));
    let mut t = scene.world.transform_mut(lamp).unwrap();
    (t.position, t.rotation) = (Vec3::Y * 6.0, Quat::from_rotation_x(-1.5));
    drop(t);
    let mut light = scene.world.light(lamp).unwrap().clone();
    (light.range, light.intensity, light.outer_cone) = (12.0, 60.0, 70.0);
    light.cast_shadows = true;
    scene.world.set_light(lamp, Some(light));
    scene
}

/// The mirrored card shades the ground as the unmirrored one does.
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
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_twins(&mut r, false, false);
}

#[test]
fn gpu_a_mirrored_plane_casts_the_spot_shadow_its_twin_does() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_twins(&mut r, true, false);
}

/// The clipped arm: a Cutout card's mirrored shadow takes the mirrored clip pipeline.
#[test]
fn gpu_a_mirrored_cutout_plane_casts_the_shadow_its_twin_does() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_twins(&mut r, false, true);
}
