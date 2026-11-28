use glam::Vec3;

use super::bottles::{eye, put, shelf, surface, H, W};
use crate::postfx::effects::taa::gpu::flicker::measure::{luma, tiles_of};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::SsrAlgorithm::{Approximation, PbrAccumulation};
use rusty_scene::create::Primitive;
use rusty_scene::{Camera, Scene};

/// Renders the two algorithms frame by frame along one path; `change` acts before frame
/// `AT`. Returns `AT`'s SSR-resolved tiles: `Approximation`'s the frame before and that
/// frame, then `PBRAccumulation`'s that frame.
fn lockstep(r: &mut Renderer, change: impl Fn(&mut Scene, &mut Camera)) -> [Vec<f32>; 3] {
    const AT: u32 = 24;
    let mut scenes = [shelf(true, Approximation), shelf(true, PbrAccumulation)].map(occluded);
    let mut views = [0; 2].map(|_| RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H));
    let (mut at, mut before) = (eye(), Vec::new());
    for f in 0..=AT {
        if f == AT {
            before = tiles_of(r, &views[0].post_fx.scene_hdr.texture, luma);
            let mut same = at.clone();
            change(&mut scenes[0], &mut at);
            change(&mut scenes[1], &mut same);
        }
        for (v, s) in views.iter_mut().zip(&scenes) {
            let target = v.color_target_view().unwrap();
            r.render(v, s, &at, &target, false);
        }
    }
    let [now, kept] = views.map(|v| tiles_of(r, &v.post_fx.scene_hdr.texture, luma));
    [before, now, kept]
}

const PLATE: (Vec3, Vec3) = (Vec3::new(-1.2, 2.3, -2.2), Vec3::new(0.8, 0.8, 0.05));

/// A satin plate in front of the shelf's left half.
fn occluded(mut s: Scene) -> Scene {
    put(
        &mut s,
        "plate",
        Primitive::Box,
        PLATE,
        surface(0.8, 1.0, 0.35),
    );
    s
}

/// The tiles the plate covers whole: it drawn glowing over a dark shelf.
fn plate_tiles(r: &mut Renderer) -> Vec<bool> {
    let mut s = shelf(false, Approximation);
    let glow = rusty_components::MaterialAsset {
        emissive: [50.0; 3],
        ..surface(0.0, 0.0, 1.0)
    };
    put(&mut s, "plate", Primitive::Box, PLATE, glow);
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let target = v.color_target_view().unwrap();
    r.render(&mut v, &s, &eye(), &target, false);
    let tiles = tiles_of(r, &v.post_fx.scene_hdr.texture, luma);
    tiles.iter().map(|&l| l > 240.0).collect()
}

fn apart(a: &[f32], b: &[f32], only: impl Fn(usize) -> bool) -> f32 {
    let gaps = (0..a.len())
        .filter(|&t| only(t))
        .map(|t| (a[t] - b[t]).abs());
    gaps.fold(0.0, f32::max)
}

#[test]
fn gpu_an_accumulated_reflection_starts_over_where_a_surface_is_revealed() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let gone = |s: &mut Scene, _: &mut Camera| {
        let plate = s.find_entity_by_name("plate").unwrap();
        s.world.transform_mut(plate).unwrap().position.y = -20.0;
    };
    let plate = plate_tiles(&mut r);
    let [before, now, kept] = lockstep(&mut r, gone);
    let revealed = |t: usize| plate[t] && (before[t] - now[t]).abs() > 4.0;
    let tiles = (0..now.len()).filter(|&t| revealed(t)).count();
    let gap = apart(&now, &kept, revealed);
    eprintln!("[ssr] over {tiles} revealed tiles, accumulated vs this frame's: {gap:.2}");
    assert!(
        tiles >= 8,
        "the plate covered the shelf: {tiles} tiles changed"
    );
    assert!(
        gap <= 1.0,
        "a revealed pixel kept the plate's history: a tile is {gap:.2} off"
    );
}

#[test]
fn gpu_an_accumulated_reflection_keeps_no_trail_when_the_camera_jumps() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let jump = |_: &mut Scene, at: &mut Camera| at.position += Vec3::new(0.8, 0.1, -0.3);
    let [_, now, kept] = lockstep(&mut r, jump);
    let gap = apart(&now, &kept, |_| true);
    eprintln!("[ssr] the frame after a jump, accumulated vs this frame's: {gap:.2}");
    assert!(
        gap <= 1.0,
        "an accumulated reflection trails a jump: a tile is {gap:.2} off"
    );
}
