//! Progress and cancel (#808): watching a bake never changes its bytes, the counter
//! ends at the total, and a cancelled bake returns nothing.

use std::sync::{Arc, Mutex};

use glam::Vec3;

use super::tests::{floor, settings, sun, wall};
use super::*;

fn scene() -> BakeScene {
    BakeScene {
        shadowmask: Default::default(),
        meshes: vec![floor(), wall()],
        lights: vec![sun(true)],
        sky: Vec3::new(0.3, 0.4, 0.5).into(),
        diffuse: Default::default(),
    }
}

/// Every texel of every map as raw bits, so "equal" means byte for byte.
fn bits(maps: &[Lightmap]) -> Vec<(u32, u32, Vec<[u32; 3]>)> {
    maps.iter()
        .map(|m| {
            let texels = m.texels.iter().map(|t| t.to_array().map(f32::to_bits));
            (m.entity, m.size, texels.collect())
        })
        .collect()
}

#[test]
fn a_watched_bake_is_byte_identical_to_an_unwatched_one() {
    let progress = BakeProgress::default();
    let watched = bake_with_progress(&scene(), &settings(), &progress).expect("not cancelled");
    let plain = bake(&scene(), &settings());
    assert!(!plain.is_empty());
    assert_eq!(bits(&watched), bits(&plain));
}

/// An observer that counts what it hears: the meshes, and each chart's batches
/// started, finished and the texels those finished.
#[derive(Default)]
struct Counting {
    meshes: Mutex<Vec<(u32, u64)>>,
    batches: Mutex<Vec<(usize, usize, usize)>>,
}

impl Counting {
    fn chart(&self, chart: usize, add: impl FnOnce(&mut (usize, usize, usize))) {
        let mut batches = self.batches.lock().unwrap();
        if batches.len() <= chart {
            batches.resize(chart + 1, (0, 0, 0));
        }
        add(&mut batches[chart]);
    }
}

impl BakeObserver for Counting {
    fn meshes(&self, meshes: &[(u32, u64)]) {
        self.meshes.lock().unwrap().extend_from_slice(meshes);
    }
    fn batch_started(&self, chart: usize) {
        self.chart(chart, |b| b.0 += 1);
    }
    fn batch_finished(&self, chart: usize, texels: usize) {
        self.chart(chart, |b| (b.1, b.2) = (b.1 + 1, b.2 + texels));
    }
}

#[test]
fn an_observed_bake_reports_every_batch_and_keeps_its_bytes() {
    let counting = Arc::new(Counting::default());
    let observed = BakeProgress::observed(counting.clone());
    let maps = bake_with_progress(&scene(), &settings(), &observed).expect("not cancelled");
    assert_eq!(bits(&maps), bits(&bake(&scene(), &settings())));
    let meshes = counting.meshes.lock().unwrap().clone();
    let entities: Vec<u32> = maps.iter().map(|m| m.entity).collect();
    assert_eq!(meshes.iter().map(|m| m.0).collect::<Vec<_>>(), entities);
    let batches = counting.batches.lock().unwrap().clone();
    assert_eq!(batches.len(), meshes.len(), "every mesh had a batch");
    for (&(started, finished, texels), &(_, size)) in batches.iter().zip(&meshes) {
        assert!(started > 0 && started == finished, "{batches:?}");
        assert_eq!(texels as u64, size, "each mesh's batches cover its texels");
    }
}

#[test]
fn progress_counts_every_texel_up_to_the_total() {
    let progress = BakeProgress::default();
    assert_eq!((progress.done(), progress.total()), (0, 0));
    let maps = bake_with_progress(&scene(), &settings(), &progress).expect("not cancelled");
    assert!(progress.total() > 0);
    assert_eq!(progress.done(), progress.total());
    // Every lightmap made counts its mesh once (#1667).
    assert_eq!(progress.meshes_total(), maps.len() as u64);
    assert_eq!(progress.meshes_done(), progress.meshes_total());
}

#[test]
fn a_cancelled_bake_returns_nothing_and_stops_tracing() {
    let progress = BakeProgress::default();
    progress.cancel();
    assert!(progress.is_cancelled());
    assert_eq!(bake_with_progress(&scene(), &settings(), &progress), None);
    assert!(
        progress.total() > 0,
        "the total is known before tracing starts"
    );
    assert_eq!(progress.done(), 0, "no texel traced after the cancel");
}
