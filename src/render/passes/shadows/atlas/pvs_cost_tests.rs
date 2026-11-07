//! What a light's PVS mask saves (#1714, part 4), measured by hand in a release
//! build: `cargo nextest run --release --features dev --run-ignored only -E
//! 'test(gpu_shadow_pvs_cost)' --no-capture` (the Dust2-sized bake takes some
//! twelve minutes on 4 threads: past nextest's five, so give the run a config with
//! a longer `slow-timeout`). A measurement, never a gate. On the bench levels,
//! three shadowed lamps a room, each swinging a little every frame so every tile
//! re-bakes, then still: casters drawn per tile and the gather's CPU time, with the
//! PVS and without it.

use std::sync::Arc;

use glam::Vec3;

use super::static_tests::RES;
use crate::render::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::lighting::occlusion::{bake_occlusion, bench_level::rooms};
use crate::scene::{Camera, Scene};

/// The bench level, `n × n` rooms, with three shadowed lamps of `range` in each
/// ground room.
fn lit(n: usize, range: f32) -> (Scene, Vec<(u32, Vec3)>) {
    let mut scene = rooms(n);
    let mut lamps = Vec::new();
    for (i, j) in (0..n).flat_map(|i| (0..n).map(move |j| (i, j))) {
        for at in [
            Vec3::new(2.5, 3.0, 2.5),
            Vec3::new(7.5, 3.0, 4.0),
            Vec3::new(5.0, 3.0, 8.0),
        ] {
            let at = Vec3::new(i as f32 * 10.0, 0.0, j as f32 * 10.0) + at;
            let id = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));
            scene.world.transform_mut(id).unwrap().position = at;
            let mut light = scene.world.light(id).unwrap().clone();
            (light.cast_shadows, light.range, light.intensity) = (true, range, 20.0);
            scene.world.set_light(id, Some(light));
            lamps.push((id, at));
        }
    }
    (scene, lamps)
}

/// `frames` frames of `scene` from a ground room's corner, the lamps swung by
/// `swing` metres each frame; the counters summed.
fn run(r: &mut Renderer, scene: &mut Scene, lamps: &[(u32, Vec3)], swing: f32) -> RenderCounters {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(11.0, 1.7, 11.0), 45.0, -5.0);
    let mut sum = RenderCounters::default();
    for frame in 0..11 {
        for &(id, at) in lamps {
            let dx = swing * (frame as f32 * 0.7).sin();
            scene.world.transform_mut(id).unwrap().position = at + Vec3::new(dx, 0.0, 0.0);
        }
        r.render(&mut view, scene, &cam, &out, false);
        let c = r.frame_counters;
        if frame > 0 {
            sum.shadow_atlas_tiles += c.shadow_atlas_tiles;
            sum.shadow_atlas_rebaked += c.shadow_atlas_rebaked;
            sum.shadow_cull_tested += c.shadow_cull_tested;
            sum.shadow_frustum_culled += c.shadow_frustum_culled;
            sum.shadow_size_culled += c.shadow_size_culled;
            sum.shadow_occlusion_culled += c.shadow_occlusion_culled;
            sum.shadow_gather_us += c.shadow_gather_us;
        }
    }
    sum
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn gpu_shadow_pvs_cost() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    for n in [5, 20] {
        let t = std::time::Instant::now();
        let data = bake_occlusion(&lit(n, 9.0).0, 4)
            .0
            .expect("the level occludes");
        let (bake, data) = (t.elapsed().as_secs_f64(), Arc::new(data));
        // A lamp's range is no part of what the bake reads: one bake serves both.
        for range in [9.0, 18.0] {
            let ((mut culled, lamps), (mut open, _)) = (lit(n, range), lit(n, range));
            *culled.occlusion = Some(Arc::clone(&data));
            for (name, swing) in [("moving", 0.05), ("still", 0.0)] {
                for (pvs, scene) in [("PVS", &mut culled), ("none", &mut open)] {
                    let c = run(&mut r, scene, &lamps, swing);
                    report(
                        &format!("n={n} ({bake:.0} s bake) {range} m {name} lamps, {pvs}"),
                        &c,
                    );
                }
            }
        }
    }
}

/// One run's line: tiles a frame, and per re-baked tile the casters tested, drawn
/// and PVS-culled, and the gather's CPU time a frame (ten frames summed in `c`).
fn report(head: &str, c: &RenderCounters) {
    let rebaked = c.shadow_atlas_rebaked.max(1) as f32;
    let culled = c.shadow_frustum_culled + c.shadow_size_culled + c.shadow_occlusion_culled;
    println!(
        "{head}: {:.1} tiles/frame, {:.1} re-baked; per re-baked tile {:.1} tested, \
         {:.1} drawn, {:.1} PVS-culled; gather {:.0} µs/frame",
        c.shadow_atlas_tiles as f32 / 10.0,
        c.shadow_atlas_rebaked as f32 / 10.0,
        c.shadow_cull_tested as f32 / rebaked,
        (c.shadow_cull_tested - culled) as f32 / rebaked,
        c.shadow_occlusion_culled as f32 / rebaked,
        c.shadow_gather_us as f32 / 10.0,
    );
}
