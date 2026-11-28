use glam::Vec3;

use super::bottles::{eye, put, shelf, surface, H, W};
use crate::postfx::effects::taa::gpu::flicker::measure::{luma, tiles_of};
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::SsrAlgorithm::{Approximation, PbrAccumulation};
use rusty_components::{MaterialAsset, SsrAlgorithm};
use rusty_scene::create::Primitive;
use rusty_scene::Scene;

const FRAMES: u32 = 12;
/// Measured on lavapipe: 3.3, and 56.5 with the door's poses unrecorded.
const TRAIL: f32 = 8.0;
/// Measured on lavapipe: 96.
const DOOR_TILES: usize = 40;
/// A frame's slide: past speed rejection's 8 px at the door's distance.
const SLIDE: f32 = 0.15;

/// The shelf with TAA off (so nothing else wants motion vectors) and a satin metal door
/// sliding sideways in front of it, a glowing handle on its face that it reflects.
fn sliding(algorithm: SsrAlgorithm, m: MaterialAsset) -> Scene {
    let mut s = shelf(true, algorithm);
    let cam = s.find_entity_by_name("Camera").unwrap();
    let mut camera = s.world.camera(cam).unwrap().clone();
    camera.taa_active = false;
    s.world.set_camera(cam, Some(camera));
    let door = (Vec3::ZERO, Vec3::new(0.8, 0.8, 0.05));
    put(&mut s, "door", Primitive::Box, door, m);
    let glow = MaterialAsset {
        emissive: [20.0; 3],
        ..surface(0.0, 0.0, 1.0)
    };
    let handle = (Vec3::ZERO, Vec3::new(0.04, 0.4, 0.04));
    put(&mut s, "handle", Primitive::Box, handle, glow);
    s
}

fn slide(s: &mut Scene, frames: u32) {
    let at = Vec3::new(-2.0 + SLIDE * frames as f32, 2.3, -2.2);
    for (name, z) in [("door", 0.0), ("handle", 0.12)] {
        let id = s.find_entity_by_name(name).unwrap();
        s.world.transform_mut(id).unwrap().position = at + Vec3::Z * z;
    }
}

/// The tiles the door covers whole on the last frame: it drawn glowing.
fn door_tiles(r: &mut Renderer) -> Vec<bool> {
    let glow = MaterialAsset {
        emissive: [50.0; 3],
        ..surface(0.0, 0.0, 1.0)
    };
    let mut s = sliding(Approximation, glow);
    slide(&mut s, FRAMES - 1);
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let target = v.color_target_view().unwrap();
    r.render(&mut v, &s, &eye(), &target, false);
    let tiles = tiles_of(r, &v.post_fx.scene_hdr.texture, luma);
    tiles.iter().map(|&l| l > 240.0).collect()
}

/// The last frame's SSR-resolved tiles under each algorithm, rendered frame by frame together.
fn last_frames(r: &mut Renderer) -> [Vec<f32>; 2] {
    let satin = surface(0.8, 1.0, 0.35);
    let mut scenes = [Approximation, PbrAccumulation].map(|a| sliding(a, satin.clone()));
    let mut views = [0; 2].map(|_| RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H));
    for f in 0..FRAMES {
        for (v, s) in views.iter_mut().zip(&mut scenes) {
            slide(s, f);
            let target = v.color_target_view().unwrap();
            r.render(v, s, &eye(), &target, false);
        }
    }
    views.map(|v| tiles_of(r, &v.post_fx.scene_hdr.texture, luma))
}

#[test]
fn gpu_a_moving_reflective_object_leaves_no_trail_with_taa_off() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let door = door_tiles(&mut r);
    let [now, kept] = last_frames(&mut r);
    let on_door = (0..now.len()).filter(|&t| door[t]);
    let tiles = on_door.clone().count();
    let gap = on_door.fold(0.0f32, |m, t| m.max((now[t] - kept[t]).abs()));
    eprintln!("[ssr] over {tiles} tiles of a sliding door, accumulated vs this frame's: {gap:.2}");
    assert!(tiles >= DOOR_TILES, "the door covers {tiles} tiles");
    assert!(
        gap <= TRAIL,
        "the door's reflection trails it: a tile is {gap:.2} off"
    );
}
