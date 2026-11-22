use std::sync::Arc;

use glam::Vec3;

use super::static_tests::RES;
use crate::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_lighting::occlusion::{bake_occlusion, bench::level::rooms};
use rusty_scene::create::{create_live_light, Primitive};
use rusty_scene::{Camera, Scene};

pub(super) fn lit(n: usize, range: f32) -> (Scene, Vec<(u32, Vec3)>) {
    let mut scene = rooms(n);
    let mut lamps = Vec::new();
    for (i, j) in (0..n).flat_map(|i| (0..n).map(move |j| (i, j))) {
        for at in [
            Vec3::new(2.5, 3.0, 2.5),
            Vec3::new(7.5, 3.0, 4.0),
            Vec3::new(5.0, 3.0, 8.0),
        ] {
            let at = Vec3::new(i as f32 * 10.0, 0.0, j as f32 * 10.0) + at;
            let id = create_live_light(&mut scene, "Lamp", Primitive::PointLight);
            scene.world.transform_mut(id).unwrap().position = at;
            let mut light = scene.world.light(id).unwrap().clone();
            (light.cast_shadows, light.range, light.intensity) = (true, range, 20.0);
            scene.world.set_light(id, Some(light));
            lamps.push((id, at));
        }
    }
    (scene, lamps)
}

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
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    for n in [5, 20] {
        let t = std::time::Instant::now();
        let data = bake_occlusion(&lit(n, 9.0).0, 4)
            .0
            .expect("the level occludes");
        let (bake, data) = (t.elapsed().as_secs_f64(), Arc::new(data));
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
