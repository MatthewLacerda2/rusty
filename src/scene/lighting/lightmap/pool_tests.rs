//! The bake's work pool (#1656): results come back in chart and item order whatever
//! the thread count, every chart is finished once with all its items, and a cancel
//! stops it; a whole bake is byte-identical on one worker or many.

use glam::Vec3;

use super::bake::bake_on;
use super::bake::pool::{run, BATCH};
use super::raster::{lightmap_size, rasterize};
use super::tests::{floor, quad, settings, sun, wall};
use super::*;

/// Charts of awkward sizes: empty, one item, a batch exactly, a batch and one more.
fn charts() -> Vec<Vec<usize>> {
    [0, 1, BATCH, BATCH + 1, 3 * BATCH + 7, 5]
        .iter()
        .map(|&n| (0..n).collect())
        .collect()
}

fn pooled(threads: usize, progress: &BakeProgress) -> Option<Vec<(usize, Vec<usize>)>> {
    let trace = |c: usize, first: usize, items: &[usize]| {
        assert!(items.len() <= BATCH);
        let at = |(k, &item): (usize, &usize)| {
            assert_eq!(item, first + k, "items arrive in order");
            c * 1000 + item
        };
        items.iter().enumerate().map(at).collect()
    };
    let finish = |c: usize, items: Vec<usize>, out: Vec<usize>| {
        assert_eq!(items, charts()[c], "a chart's own items come back to it");
        (c, out)
    };
    run(charts(), threads, progress, trace, finish)
}

#[test]
fn results_keep_chart_and_item_order_on_any_thread_count() {
    let expect: Vec<(usize, Vec<usize>)> = charts()
        .iter()
        .enumerate()
        .map(|(c, items)| (c, items.iter().map(|i| c * 1000 + i).collect()))
        .collect();
    for threads in [0, 1, 2, 7, 64] {
        let progress = BakeProgress::default();
        assert_eq!(
            pooled(threads, &progress),
            Some(expect.clone()),
            "{threads}"
        );
        assert_eq!(progress.done(), 5 * BATCH as u64 + 14, "{threads}");
    }
}

#[test]
fn no_charts_is_an_empty_result_and_a_cancel_is_none() {
    let nothing = BakeProgress::default();
    let none = run::<u8, u8, u8>(vec![], 4, &nothing, |_, _, _| vec![], |_, _, _| 0);
    assert_eq!(none, Some(vec![]));
    let progress = BakeProgress::default();
    progress.cancel();
    assert_eq!(pooled(4, &progress), None);
    assert_eq!(progress.done(), 0);
}

#[test]
fn a_bake_is_byte_identical_on_one_worker_or_many() {
    // Many small meshes, so batches from several charts are in flight at once.
    let mut meshes = vec![floor(), wall()];
    for k in 0..12 {
        let (x, y) = (-1.5 + 0.25 * k as f32, 0.1 + 0.2 * k as f32);
        let p = |dx: f32, dz: f32| Vec3::new(x + dx, y, dz);
        let c = [p(0.0, 0.5), p(0.5, 0.5), p(0.5, 0.0), p(0.0, 0.0)];
        meshes.push(quad(10 + k, c, Vec3::splat(0.7)));
    }
    let scene = BakeScene {
        meshes,
        lights: vec![sun(true)],
        sky: Vec3::new(0.3, 0.4, 0.5).into(),
    };
    // Dense enough that the floor and wall span several batches, the last partial.
    let s = BakeSettings {
        texels_per_unit: 5.0,
        max_resolution: 32,
        samples: 8,
        filter_radius: 2,
        ..settings()
    };
    let texels = rasterize(
        &scene.meshes[0],
        lightmap_size(&scene.meshes[0], 5.0, 32).unwrap(),
    );
    assert!(
        texels.len() > 2 * BATCH && !texels.len().is_multiple_of(BATCH),
        "{}",
        texels.len()
    );
    let on = |threads| bake_on(&scene, &s, &BakeProgress::default(), threads);
    let one = on(1).expect("not cancelled");
    assert_eq!(one.len(), 14);
    let bits = |maps: &[Lightmap]| -> Vec<u32> {
        let all = maps
            .iter()
            .flat_map(|m| m.texels.iter().chain(&m.directions));
        all.flat_map(|t| t.to_array().map(f32::to_bits)).collect()
    };
    for threads in [3, 16] {
        assert_eq!(
            bits(&on(threads).expect("not cancelled")),
            bits(&one),
            "{threads}"
        );
    }
}
