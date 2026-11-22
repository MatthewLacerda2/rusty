use glam::{Quat, Vec3};

use super::mirror_tests::{spot_yard, sun_yard};
use crate::gpu::pipelines::surface::cut_depth_tests::{ground_at, ground_halves};
use crate::lod::LodSelection;
use crate::{Frustum, Renderer};
use rusty_components::{RenderMode, ShadowCasting};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) fn card(scene: &mut Scene, mode: ShadowCasting, face_down: bool, cutout: bool) -> u32 {
    let plane = create_entity(scene, "Card", Some(Primitive::Plane));
    let mut t = scene.world.transform_mut(plane).unwrap();
    (t.position, t.scale) = (Vec3::Y * 3.0, Vec3::new(0.4, 1.0, 0.4));
    if face_down {
        t.rotation = Quat::from_rotation_x(std::f32::consts::PI);
    }
    drop(t);
    scene.world.mesh_mut(plane).unwrap().cast_shadows = mode;
    if cutout {
        let key = scene.ensure_material_key(plane).unwrap();
        scene.materials.get_mut(&key).unwrap().render_mode = RenderMode::Cutout;
        scene.materials.get_mut(&key).unwrap().alpha_cutoff = 0.5;
    }
    plane
}

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
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_modes(&mut r, false);
}

#[test]
fn gpu_cast_shadows_modes_shade_under_a_spotlight() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    assert_modes(&mut r, true);
}

#[test]
fn gpu_a_static_card_turned_off_rebakes_and_a_cutout_casts_two_sided() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let open = ground_halves(&mut r, &sun_yard());
    let mut scene = sun_yard();
    let id = card(&mut scene, ShadowCasting::On, false, false);
    scene.world.set_static(id, true);
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

#[test]
fn gpu_a_shadows_only_card_is_drawn_by_no_camera() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 10.0, 0.0), -90.0, -89.0);
    let mut visible = |mode| {
        let mut scene = sun_yard();
        card(&mut scene, mode, false, false);
        ground_halves(&mut r, &scene);
        let frustum = Frustum::from_view_proj(cam.build_view_projection(1.0));
        let lod = LodSelection::for_camera(&scene, &cam);
        let (culled, marks) = ((&frustum, &lod), (false, None, Default::default()));
        let solids = r.precreate_solid_resources(&scene, &cam, culled, marks);
        solids.visible
    };
    let on = visible(ShadowCasting::On);
    assert_eq!(
        visible(ShadowCasting::ShadowsOnly) + 1,
        on,
        "the card alone is hidden"
    );
}
