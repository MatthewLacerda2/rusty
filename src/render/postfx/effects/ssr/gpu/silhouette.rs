//! SSR's march refined below a step (#2004): a thin pole 0.3 m in front of a mirror
//! reflects with a straight silhouette, every row, where a fixed step drew stairs
//! and dashes. The scene and frame helpers the rough-edge test (`edge.rs`) shares.

use glam::Vec3;

use super::fixture::{paint, place, tuned, Surface, RED};
use crate::core::settings::quality::EffectQuality;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const W: u32 = 320;
pub(super) const H: u32 = 240;
pub(super) const BRONZE: [f32; 3] = [0.8, 0.55, 0.35];
const GREEN: Surface = ([0.1, 0.9, 0.1], 0.0, 1.0);

/// The fixture's scene, SSR `on` at the issue's march (64 steps of 0.3 m), the
/// floor `floor`.
pub(super) fn base(on: bool, floor: Surface) -> Scene {
    let (s, _) = tuned(on, floor, RED, |v| {
        v.ssr.quality = EffectQuality::Custom;
        (v.ssr.steps, v.ssr.step_length) = (64, 0.3);
    });
    s
}

/// A bronze mirror wall whose face is 0.3 m behind a thin green pole, the cube
/// moved out of sight.
fn mirror(on: bool) -> Scene {
    let mut s = base(on, ([0.5; 3], 0.0, 1.0));
    let cube = s.find_entity_by_name("Cube").unwrap();
    s.world.transform_mut(cube).unwrap().position.x = 50.0;
    let pole = create_entity(&mut s, "Pole", Some(Primitive::Box));
    place(
        &mut s,
        pole,
        Vec3::new(0.0, 1.0, -5.0),
        Vec3::new(0.08, 2.4, 0.08),
    );
    s.world.transform_mut(pole).unwrap().rotation = glam::Quat::from_rotation_z(0.35);
    paint(&mut s, pole, "pole", GREEN);
    let wall = create_entity(&mut s, "Mirror", Some(Primitive::Box));
    place(
        &mut s,
        wall,
        Vec3::new(0.0, 1.5, -5.38),
        Vec3::new(8.0, 3.0, 0.08),
    );
    paint(&mut s, wall, "mirror", (BRONZE, 1.0, 0.04));
    s
}

pub(super) fn shot(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

pub(super) fn at(f: &[u8], x: u32, y: u32) -> [i32; 3] {
    let i = ((y * W + x) * 4) as usize;
    [0, 1, 2].map(|c| i32::from(f[i + c]))
}

fn green(c: [i32; 3]) -> bool {
    c[1] > c[0] + 30 && c[1] > c[2] + 30
}

/// Per row of the frame's middle band, the leftmost and rightmost pixel green in
/// `on` but not in `off`: the pole's reflection.
fn reflection_rows(on: &[u8], off: &[u8]) -> Vec<Option<(u32, u32)>> {
    (H / 2 - 40..H / 2 + 40)
        .map(|y| {
            let xs: Vec<u32> = (0..W)
                .filter(|&x| green(at(on, x, y)) && !green(at(off, x, y)))
                .collect();
            Some((*xs.first()?, *xs.last()?))
        })
        .collect()
}

#[test]
fn gpu_a_pole_close_to_a_mirror_reflects_with_a_straight_silhouette() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let side = Camera::new(Vec3::new(-2.5, 1.0, -2.0), -60.0, 0.0);
    let (on, off) = (
        shot(&mut r, &mirror(true), &side),
        shot(&mut r, &mirror(false), &side),
    );
    let rows = reflection_rows(&on, &off);
    let found: Vec<(f32, f32, f32)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.map(|(l, r)| (i as f32, l as f32, r as f32)))
        .collect();
    let (left, right) = (wobble(&found, |e| e.1), wobble(&found, |e| e.2));
    let own = (0..W).filter(|&x| green(at(&off, x, H / 2))).count();
    let width = found.iter().map(|e| e.2 - e.1 + 1.0).sum::<f32>() / found.len().max(1) as f32;
    eprintln!("[ssr] leaning pole: {} of {} rows reflect, {width:.1} px wide (the pole {own}), edges off a line by {left:.2} / {right:.2} px", found.len(), rows.len());
    assert_eq!(found.len(), rows.len(), "every row reflects the pole");
    assert!(
        left < 1.5 && right < 1.5,
        "a straight pole's reflection has straight sides"
    );
}

/// How far, at most, an edge strays from its least-squares line through the rows.
fn wobble(rows: &[(f32, f32, f32)], edge: fn(&(f32, f32, f32)) -> f32) -> f32 {
    let n = rows.len().max(1) as f32;
    let (my, mx) = (
        rows.iter().map(|e| e.0).sum::<f32>() / n,
        rows.iter().map(edge).sum::<f32>() / n,
    );
    let sxy: f32 = rows.iter().map(|e| (e.0 - my) * (edge(e) - mx)).sum();
    let syy: f32 = rows
        .iter()
        .map(|e| (e.0 - my).powi(2))
        .sum::<f32>()
        .max(1e-6);
    let slope = sxy / syy;
    rows.iter()
        .map(|e| (edge(e) - mx - slope * (e.0 - my)).abs())
        .fold(0.0, f32::max)
}
