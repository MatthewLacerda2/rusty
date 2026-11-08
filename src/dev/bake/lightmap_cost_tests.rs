//! The lightmap bake's cost (#1883), measured by hand in a release build:
//! `cargo nextest run --release --features dev --run-ignored only -E
//! 'test(lightmap_trace_cost)' --no-capture`. A measurement, never a gate. It
//! traces a spread of texels (`lightmap::trace_texels`) of the occlusion bench's
//! levels, lit three lamps a room (two points and an Area panel, a spot in every
//! fifth room) with a few emissive props, at the default bake settings, and
//! prints texels per second on 4 threads. The gate beside it counts rays, never
//! time: a timing is not deterministic (`docs/testing.md`).

use std::time::Instant;

use glam::Vec3;

use crate::scene::lighting::lightmap::{
    trace_texels, AreaShape, BakeLight, BakeScene, BakeSettings, LightShape,
};
use crate::scene::lighting::occlusion::bench_level::rooms;

/// The `n × n`-rooms bench level as the bake sees it, with its lamps.
fn lit_level(n: usize) -> BakeScene {
    let mut scene = BakeScene::gather(&rooms(n), &|_| None);
    let lamp = |shape, radiance: f32| BakeLight {
        shape,
        radiance: Vec3::splat(radiance),
        bakes_direct: true,
        mask_channel: None,
    };
    for (k, (storey, i, j)) in (0..2)
        .flat_map(|s| (0..n).flat_map(move |i| (0..n).map(move |j| (s, i, j))))
        .enumerate()
    {
        let top = Vec3::new(i as f32 * 10.0, storey as f32 * 4.0 + 3.4, j as f32 * 10.0);
        for at in [Vec3::new(3.0, 0.0, 3.0), Vec3::new(7.0, 0.0, 7.0)] {
            let shape = LightShape::Point {
                position: top + at,
                range: 8.0,
            };
            scene.lights.push(lamp(shape, 3.0));
        }
        let at = top + Vec3::new(5.0, 0.0, 5.0);
        let shape = match k % 5 {
            4 => LightShape::Spot {
                position: at,
                direction: Vec3::NEG_Y,
                range: 8.0,
                cos_inner: 0.9,
                cos_outer: 0.7,
            },
            _ => LightShape::Area(AreaShape {
                center: at,
                right: Vec3::X,
                up: Vec3::Z * 0.5,
                forward: Vec3::NEG_Y,
                range: 8.0,
            }),
        };
        scene.lights.push(lamp(shape, 4.0));
    }
    for mesh in scene.meshes.iter_mut().step_by(37) {
        mesh.runs[0].emissive = Vec3::splat(2.0);
    }
    scene
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn lightmap_trace_cost() {
    let settings = BakeSettings::default();
    for (name, n, count) in [("Kolvik-sized", 5, 12000), ("Dust2-sized", 20, 6000)] {
        let scene = lit_level(n);
        let t = Instant::now();
        let (traced, _) = trace_texels(&scene, &settings, count, 4);
        let secs = t.elapsed().as_secs_f64();
        println!(
            "{name}: {} meshes, {} lights, {traced} texels in {secs:.2} s, {:.0} texels/s",
            scene.meshes.len(),
            scene.lights.len(),
            traced as f64 / secs
        );
    }
}

/// The 2 × 2-rooms level with `lamps` points spread through it, every one in range
/// of most of it.
fn crowded(lamps: usize) -> BakeScene {
    let mut scene = BakeScene::gather(&rooms(2), &|_| None);
    scene.lights = (0..lamps)
        .map(|k| {
            let f = k as f32 / lamps as f32;
            let position = Vec3::new(1.0 + 18.0 * f, 3.0 + 4.0 * (k % 2) as f32, 19.0 - 18.0 * f);
            let shape = LightShape::Point {
                position,
                range: 12.0,
            };
            BakeLight {
                shape,
                radiance: Vec3::splat(3.0 / lamps as f32),
                bakes_direct: k % 3 == 0,
                mask_channel: None,
            }
        })
        .collect();
    scene
}

#[test]
fn a_hundred_and_fifty_lamps_cast_a_small_factor_of_ones_rays() {
    // The rays cast are the bake's cost: one pick per path vertex, however many
    // lamps are in range, plus the texel's own shadow ray per `Baked` lamp.
    let settings = BakeSettings {
        samples: 16,
        ..Default::default()
    };
    let rays = |lamps| trace_texels(&crowded(lamps), &settings, 96, 1).1;
    let (one, many) = (rays(1), rays(150));
    println!(
        "1 lamp {one} rays, 150 lamps {many}: {:.2}×",
        many as f64 / one as f64
    );
    assert!(
        many < one * 3,
        "150 lamps cast {many} rays against one's {one}"
    );
}
