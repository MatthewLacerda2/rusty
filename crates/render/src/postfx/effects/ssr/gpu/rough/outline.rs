//! A rough reflection's outline is as soft as its inside (#2158): a satin bronze
//! ceiling, seen from below, reflecting a lit strip on a wall behind a row of thin
//! dark uprights. Where the ceiling's rays stop finding the strip (the band's
//! edge, cut by the uprights' silhouettes) the reflection fades over the blur its
//! roughness gives the band, at any resolution; a mirror keeps a sharp edge.

use glam::Vec3;

use super::super::fixture::{paint, place, Surface};
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const FOV: f32 = 40.0;

/// A box named `name` centred at `at`, `size` across, painted `surface`.
fn block(s: &mut Scene, name: &str, at: Vec3, size: Vec3, surface: Surface) -> u32 {
    let id = create_entity(s, name, Some(Primitive::Box));
    place(s, id, at, size);
    paint(s, id, name, surface);
    id
}

/// A room under a bronze metal ceiling of `roughness` 3.4 m up, SSR on and the
/// sky black: an emissive strip 2.5 m up its back wall, 4 m out, and in front of
/// it a row of thin dark uprights 0.18 m apart.
fn bar(roughness: f32) -> Scene {
    let mut s = Scene::new();
    crate::test_gpu::black_sky(&mut s);
    let cam = s.add_entity("Camera".to_string());
    s.world.set_camera(cam, Some(Default::default()));
    s.world.camera_mut(cam).unwrap().fov = FOV;
    let mut volume = default_post_processing();
    (volume.ssr.active, volume.ssr.min_smoothness) = (true, 0.3);
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let bronze = ([0.8, 0.55, 0.35], 1.0, roughness);
    let [matte, grey, white] = [0.3, 0.2, 1.0].map(|c| ([c; 3], 0.0, 1.0));
    let room = [
        ("ceiling", 3.45, -3.0, [12.0, 0.1, 12.0], bronze),
        ("floor", -0.05, -3.0, [12.0, 0.1, 12.0], matte),
        ("wall", 1.7, -4.05, [12.0, 3.4, 0.1], grey),
        ("strip", 2.5, -3.97, [12.0, 0.15, 0.04], white),
    ];
    for (name, y, z, size, surface) in room {
        block(&mut s, name, Vec3::new(0.0, y, z), size.into(), surface);
    }
    let id = s.find_entity_by_name("strip").unwrap();
    let led = MaterialAsset {
        emissive: [24.0, 20.0, 14.0],
        ..s.materials["strip"].clone()
    };
    s.materials.insert("led".into(), led);
    s.world
        .set_material(id, Some(MaterialComponent::new("led".to_string())));
    for i in -20..=20 {
        let at = Vec3::new(i as f32 * 0.18, 2.4, -3.85);
        block(
            &mut s,
            "upright",
            at,
            Vec3::new(0.03, 0.6, 0.03),
            ([0.05; 3], 0.0, 1.0),
        );
    }
    s
}

/// The eye: 1.6 m up, looking at the back wall, pitched up 30°.
fn eye() -> Camera {
    let mut c = Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0);
    c.fov = FOV;
    c
}

/// The red of the HDR image the SSR pass resolved into, row by row, `(w, h)`.
fn red(r: &mut Renderer, scene: &Scene, (w, h): (u32, u32)) -> Vec<f32> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, w, h);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, &eye(), &out, false);
    let bytes = read_texture_bytes(
        &r.device,
        &r.queue,
        &view.post_fx.scene_hdr.texture,
        (w, h),
        8,
    );
    (0..(w * h) as usize)
        .map(|t| f16_to_f32(u16::from_le_bytes([bytes[t * 8], bytes[t * 8 + 1]])))
        .collect()
}

/// Down every column of the frame's middle, over the ceiling (its top 60%), the
/// largest share of the column's brightest red one row adds or takes: near 1 is a
/// hard edge; a fade spreads it over its width, so it halves at twice the size.
/// With it, the brightest red of any of those columns.
fn steepest(red: &[f32], (w, h): (u32, u32)) -> (f32, f32) {
    let column = |x: u32| (0..h * 6 / 10).map(move |y| red[(y * w + x) as usize]);
    (w * 3 / 10..w * 7 / 10)
        .map(|x| {
            let c: Vec<f32> = column(x).collect();
            let peak = c.iter().copied().fold(1e-3, f32::max);
            let step = c
                .windows(2)
                .map(|p| (p[1] - p[0]).abs())
                .fold(0.0, f32::max);
            (step / peak, peak)
        })
        .fold((0.0, 0.0), |a, b| (a.0.max(b.0), a.1.max(b.1)))
}

/// The least width, as a share of the frame's height, over which the satin
/// ceiling's reflection may rise: here the resolve blurs the band's inside over a
/// radius near 0.06 of the height, and an outline drawn at the rays' hit or miss
/// rose within 0.017 at 360 rows and 0.009 at 720.
const SOFT: f32 = 0.025;

/// The least share of the mirror's brightest red the satin band reaches (near
/// 0.09 here, 0 with SSR off): its blur spreads the strip's light thin, but a
/// band that is not there at all would pass `SOFT`.
const BAND: f32 = 0.03;

#[test]
fn gpu_a_satin_ceiling_reflects_a_strip_behind_uprights_with_a_soft_outline() {
    for size in [(640, 360), (1280, 720)] {
        let Some(mut r) = crate::test_gpu::headless_or_skip(size.0, size.1) else {
            return;
        };
        let (mirror, bright) = steepest(&red(&mut r, &bar(0.04), size), size);
        let (satin, peak) = steepest(&red(&mut r, &bar(0.35), size), size);
        let width = 1.0 / (satin * size.1 as f32);
        let share = peak / bright;
        eprintln!("[ssr] at {size:?}, a mirror's reflected band steps {mirror:.2} of its peak in a row; at roughness 0.35 it rises over {width:.3} of the height, as bright as {share:.2} of the mirror's");
        assert!(mirror >= 0.5, "a mirror reflects the strip's edge sharp");
        assert!(share >= BAND, "the satin ceiling reflects the strip");
        assert!(
            width >= SOFT,
            "a satin reflection's outline is as soft as its inside (#2158)"
        );
    }
}
