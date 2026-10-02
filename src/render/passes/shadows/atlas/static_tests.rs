//! The atlas's static-caster cache on the GPU (#694): a still frame re-bakes no tile
//! and draws no static caster, and its atlas is bit-identical to a forced full
//! redraw — also with a dynamic caster moving through a cached tile. Skips with no
//! adapter.

use glam::Vec3;

use super::gpu_tests::{dark, lamp};
use super::ATLAS_SIZE;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const RES: u32 = 48;

/// A box at `position` scaled by `scale`, static when `is_static`; returns its id.
fn block(scene: &mut Scene, position: Vec3, scale: Vec3, is_static: bool) -> u32 {
    let id = create_entity(scene, "Block", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    drop(t);
    scene.world.set_static(id, is_static);
    id
}

/// A static floor and pillar under a shadowing spotlight and point light (7 tiles).
fn lamp_room() -> Scene {
    let mut scene = dark();
    let floor = Vec3::new(20.0, 1.0, 20.0);
    block(&mut scene, Vec3::new(0.0, -0.5, 0.0), floor, true);
    block(&mut scene, Vec3::new(1.0, 1.0, 0.0), Vec3::splat(0.6), true);
    let down = Vec3::new(0.3, -1.0, 0.0);
    lamp(
        &mut scene,
        Primitive::SpotLight,
        Vec3::new(-1.0, 3.0, 0.0),
        down,
        true,
    );
    lamp(
        &mut scene,
        Primitive::PointLight,
        Vec3::new(0.0, 2.5, 1.5),
        down,
        true,
    );
    scene
}

/// Render one frame of `scene`; returns the active atlas's raw texels (four bytes
/// each, the Depth32Float values bit for bit).
fn frame(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> Vec<u8> {
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 6.0, 6.0), -90.0, -40.0);
    r.render(view, scene, &cam, &out, false);
    let atlas = &r.shadow_renderer.atlas.texture;
    readback::read_texture_rgba8(&r.device, &r.queue, atlas, ATLAS_SIZE, ATLAS_SIZE)
}

/// `(cached, rebaked)` tiles of the last frame.
fn bake(r: &Renderer) -> (u32, u32) {
    let c = &r.frame_counters;
    (c.shadow_atlas_cached, c.shadow_atlas_rebaked)
}

/// Texels nearer than the far plane: what the casters drew.
fn drawn(atlas: &[u8]) -> usize {
    let far = 1.0f32.to_le_bytes();
    atlas.chunks_exact(4).filter(|t| *t != far).count()
}

#[test]
fn gpu_a_still_frame_reuses_every_tile_bit_for_bit() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let scene = lamp_room();

    let first = frame(&mut r, &mut view, &scene);
    assert_eq!(r.frame_counters.shadow_atlas_tiles, 7);
    assert_eq!(bake(&r), (0, 7), "the first frame bakes every tile");
    assert!(drawn(&first) > 0, "the statics reached the atlas");

    let cached = frame(&mut r, &mut view, &scene);
    assert_eq!(bake(&r), (7, 0), "still: every tile cached");
    assert_eq!(r.frame_counters.shadow_draws, 0, "no static caster redrawn");

    r.shadow_renderer.invalidate_static_cache();
    let redrawn = frame(&mut r, &mut view, &scene);
    assert_eq!(bake(&r), (0, 7), "invalidated: a full redraw");
    assert!(cached == redrawn, "a cached frame equals a full redraw");
    assert!(first == cached);
}

#[test]
fn gpu_a_dynamic_caster_moves_through_a_cached_tile() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let mut scene = lamp_room();
    let start = Vec3::new(-1.0, 1.0, 0.0);
    let crate_id = block(&mut scene, start, Vec3::splat(0.4), false);
    let statics = frame(&mut r, &mut view, &lamp_room());

    let before = frame(&mut r, &mut view, &scene);
    assert!(before != statics, "the dynamic caster is drawn");
    scene.world.transform_mut(crate_id).unwrap().position = start + Vec3::Z;
    let moved = frame(&mut r, &mut view, &scene);
    assert_eq!(bake(&r), (7, 0), "a dynamic caster re-bakes no tile");
    assert!(r.frame_counters.shadow_draws > 0, "but is redrawn");
    assert!(before != moved, "the atlas follows it");

    r.shadow_renderer.invalidate_static_cache();
    let redrawn = frame(&mut r, &mut view, &scene);
    assert_eq!(bake(&r), (0, 7));
    assert!(
        moved == redrawn,
        "cached statics + moved caster equal a full redraw"
    );
}
