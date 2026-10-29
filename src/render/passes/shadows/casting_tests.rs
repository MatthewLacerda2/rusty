//! A mesh's `cast_shadows` (#1571), Unity's *Cast Shadows*, honoured by the sun's
//! cascades and the spot/point atlas: `Off` casts nothing, `ShadowsOnly` casts as
//! `On` though no camera draws it, and `TwoSided` casts from a one-sided `Plane`
//! that faces away from the light, which back-face culling drops otherwise, plain
//! or Cutout (the clip pipeline's twin). A static caster's cached bake follows a
//! changed mode with no static flag touched. Skips with no adapter.

use glam::{Quat, Vec3};

use super::mirror_tests::{spot_yard, sun_yard};
use crate::components::{RenderMode, ShadowCasting};
use crate::render::gpu::pipelines::surface::cut_depth_tests::{ground_at, ground_halves};
use crate::render::lod::LodSelection;
use crate::render::{Frustum, Renderer};
use crate::scene::authoring::{create_entity, material, Primitive};
use crate::scene::{Camera, Scene};

/// A 6 x 6 one-sided `Plane` at y = 3 casting by `mode`; `face_down` turns its
/// front away from the light overhead, `cutout` gives it a solid Cutout material.
fn card(scene: &mut Scene, mode: ShadowCasting, face_down: bool, cutout: bool) -> u32 {
    let plane = create_entity(scene, "Card", Some(Primitive::Plane));
    let mut t = scene.world.transform_mut(plane).unwrap();
    (t.position, t.scale) = (Vec3::Y * 3.0, Vec3::new(0.4, 1.0, 0.4));
    if face_down {
        t.rotation = Quat::from_rotation_x(std::f32::consts::PI);
    }
    drop(t);
    scene.world.mesh_mut(plane).unwrap().cast_shadows = mode;
    if cutout {
        let key = material::ensure_material_key(scene, plane).unwrap();
        material::set_render_mode(&mut scene.materials, &key, RenderMode::Cutout);
        material::set_alpha_cutoff(&mut scene.materials, &key, 0.5);
    }
    plane
}

/// The ground's brightness under each half of the card (none for `None`), lit by
/// the sun or, with `spot`, by a shadowing spotlight overhead.
fn shade(r: &mut Renderer, mode: Option<ShadowCasting>, spot: bool, down: bool) -> [f32; 2] {
    let mut scene = if spot { spot_yard() } else { sun_yard() };
    if let Some(mode) = mode {
        card(&mut scene, mode, down, false);
    }
    ground_halves(r, &scene)
}

fn shaded(lit: [f32; 2], open: [f32; 2]) -> bool {
    (0..2).all(|i| lit[i] + 20.0 < open[i])
}

fn alike(a: [f32; 2], b: [f32; 2]) -> bool {
    (0..2).all(|i| (a[i] - b[i]).abs() < 5.0)
}

fn assert_modes(r: &mut Renderer, spot: bool) {
    let open = shade(r, None, spot, false);
    let [on, off, only] = [
        ShadowCasting::On,
        ShadowCasting::Off,
        ShadowCasting::ShadowsOnly,
    ]
    .map(|mode| shade(r, Some(mode), spot, false));
    let away = shade(r, Some(ShadowCasting::On), spot, true);
    let both = shade(r, Some(ShadowCasting::TwoSided), spot, true);
    eprintln!("[cast shadows, spot {spot}] open {open:?} on {on:?} off {off:?} only {only:?}");
    eprintln!("[cast shadows, spot {spot}] facing away: one-sided {away:?}, two-sided {both:?}");
    assert!(shaded(on, open), "control: the card shades");
    assert!(alike(off, open), "Off casts nothing");
    assert!(alike(only, on), "ShadowsOnly casts as On");
    assert!(
        alike(away, open),
        "control: a card facing away casts nothing"
    );
    assert!(alike(both, on), "TwoSided casts from its back face");
}

#[test]
fn gpu_cast_shadows_modes_shade_under_the_sun() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_modes(&mut r, false);
}

#[test]
fn gpu_cast_shadows_modes_shade_under_a_spotlight() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_modes(&mut r, true);
}

/// A static card's shadow is baked once and cached; turning it `Off` re-bakes it
/// though no static flag moved. A two-sided Cutout card draws through the clip
/// pipeline's unculled twin.
#[test]
fn gpu_a_static_card_turned_off_rebakes_and_a_cutout_casts_two_sided() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let open = ground_halves(&mut r, &sun_yard());
    let mut scene = sun_yard();
    let id = card(&mut scene, ShadowCasting::On, false, false);
    scene.world.set_static(id, true);
    // One camera throughout, so the cascades keep their volumes and the bake its cache.
    let [baked, cached] = [(); 2].map(|()| ground_at(&mut r, &scene, -1.5));
    scene.world.mesh_mut(id).unwrap().cast_shadows = ShadowCasting::Off;
    let off = ground_at(&mut r, &scene, -1.5);
    let mut cut = sun_yard();
    card(&mut cut, ShadowCasting::TwoSided, true, true);
    let cut = ground_halves(&mut r, &cut);
    eprintln!(
        "[cast shadows] open {open:?} static {baked} {cached} then off {off}; cutout {cut:?}"
    );
    assert!(
        baked + 20.0 < open[0] && cached == baked,
        "control: the static card shades"
    );
    assert!(
        (off - open[0]).abs() < 5.0,
        "the cached bake follows the mode"
    );
    assert!(shaded(cut, open), "a two-sided Cutout casts from its back");
}

/// No camera draws a `ShadowsOnly` mesh.
#[test]
fn gpu_a_shadows_only_card_is_drawn_by_no_camera() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 10.0, 0.0), -90.0, -89.0);
    let mut visible = |mode| {
        let mut scene = sun_yard();
        card(&mut scene, mode, false, false);
        ground_halves(&mut r, &scene); // makes the meshes resident
        let frustum = Frustum::from_view_proj(cam.build_view_projection(1.0));
        let lod = LodSelection::for_camera(&scene, &cam);
        let solids = r.precreate_solid_resources(&scene, &cam, &frustum, &lod, (false, None));
        solids.visible
    };
    let on = visible(ShadowCasting::On);
    assert_eq!(
        visible(ShadowCasting::ShadowsOnly) + 1,
        on,
        "the card alone is hidden"
    );
}
