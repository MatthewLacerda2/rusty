//! The bake's denoiser choice (#1884): `None` leaves the bounce raw, an `Oidn` bake
//! with no denoiser lent (or one that fails) is the `Filter` bake to the byte, a lent
//! denoiser's output is what the lightmap keeps (bad texels excepted), and a bake
//! through one is byte-identical on one worker or many. The mocks stand in for OIDN;
//! the real one is tested beside its binding (`dev::bake::denoise`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use glam::Vec3;

use super::bake::bake_on;
use super::tests::{floor, settings, sun, wall};
use super::*;

/// What a mock does to each image it is handed.
#[derive(Clone, Copy)]
enum Mock {
    /// Every texel set to the image's mean: a perfect flat denoise.
    Flatten,
    /// Refuses.
    Fail,
    /// Every texel NaN.
    Poison,
}

struct Lent(Mock, AtomicUsize);

impl LightmapDenoiser for Lent {
    fn denoise(&self, image: &mut [Vec3], size: u32, _: bool) -> Result<(), String> {
        assert_eq!(image.len(), (size * size) as usize, "a whole chart");
        self.1.fetch_add(1, Ordering::Relaxed);
        let mean = image.iter().sum::<Vec3>() / image.len() as f32;
        let to = match self.0 {
            Mock::Flatten => mean,
            Mock::Fail => return Err("refused".into()),
            Mock::Poison => Vec3::NAN,
        };
        image.fill(to);
        Ok(())
    }
}

fn scene() -> BakeScene {
    BakeScene {
        shadowmask: Default::default(),
        meshes: vec![floor(), wall()],
        lights: vec![sun(false)],
        sky: Vec3::new(0.3, 0.4, 0.5).into(),
        diffuse: Default::default(),
    }
}

fn with(denoiser: Denoiser, radius: u32) -> BakeSettings {
    BakeSettings {
        denoiser,
        filter_radius: radius,
        ..settings()
    }
}

/// A bake on `threads` workers, lent `mock` when there is one; and how many images
/// it was handed.
fn baked(s: &BakeSettings, mock: Option<Mock>, threads: usize) -> (Vec<Lightmap>, usize) {
    let lent = mock.map(|m| Arc::new(Lent(m, AtomicUsize::new(0))));
    let progress = match &lent {
        Some(l) => BakeProgress::default().with_denoiser(l.clone()),
        None => BakeProgress::default(),
    };
    let maps = bake_on(&scene(), s, &progress, threads).expect("not cancelled");
    (maps, lent.map_or(0, |l| l.1.load(Ordering::Relaxed)))
}

#[test]
fn none_is_the_raw_trace_and_oidn_without_a_denoiser_is_the_filter() {
    let raw = baked(&with(Denoiser::Filter, 0), None, 2).0;
    assert_eq!(
        baked(&with(Denoiser::None, 3), None, 2).0,
        raw,
        "None ignores the radius"
    );
    let filtered = baked(&with(Denoiser::Filter, 2), None, 2).0;
    assert_ne!(filtered, raw);
    assert_eq!(baked(&with(Denoiser::Oidn, 2), None, 2).0, filtered);
    let (failed, calls) = baked(&with(Denoiser::Oidn, 2), Some(Mock::Fail), 2);
    assert_eq!(failed, filtered, "a failed denoise filters");
    assert_eq!(
        calls, 2,
        "tried once per chart, the directions never reached"
    );
}

#[test]
fn a_lent_denoiser_cleans_the_bounce_and_its_directions() {
    let filtered = baked(&with(Denoiser::Filter, 2), None, 2).0;
    let (clean, calls) = baked(&with(Denoiser::Oidn, 2), Some(Mock::Flatten), 2);
    assert_eq!(calls, 4, "the bounce and its directions, per chart");
    assert_ne!(clean, filtered);
    // Only the bounce was flattened: no Baked lamp, so the whole chart is one value.
    let floor = &clean[0];
    let first = floor.texels[0];
    assert!(floor
        .texels
        .iter()
        .all(|t| (*t - first).abs().max_element() < 1e-5));
    for d in &floor.directions {
        assert!(d.truncate().length() <= 1.0 + 1e-5, "{d}");
    }
    // A Filter bake leaves the lent denoiser alone.
    assert_eq!(
        baked(&with(Denoiser::Filter, 2), Some(Mock::Flatten), 2).1,
        0
    );
}

#[test]
fn a_denoiser_that_returns_nan_keeps_the_trace() {
    let raw = baked(&with(Denoiser::None, 0), None, 2).0;
    let poisoned = baked(&with(Denoiser::Oidn, 2), Some(Mock::Poison), 2).0;
    for (p, r) in poisoned.iter().zip(&raw) {
        assert_eq!(p.texels, r.texels, "the colour is the raw trace");
        assert!(p.directions.iter().all(|d| d.is_finite()));
    }
}

#[test]
fn a_denoised_bake_is_byte_identical_on_one_worker_or_many() {
    let s = with(Denoiser::Oidn, 2);
    let one = baked(&s, Some(Mock::Flatten), 1).0;
    assert_eq!(baked(&s, Some(Mock::Flatten), 5).0, one);
}

#[test]
fn a_file_without_a_denoiser_reads_oidn_and_names_round_trip() {
    let old: BakeSettings = serde_json::from_str(r#"{"samples": 64}"#).expect("parses");
    assert_eq!((old.denoiser, old.samples), (Denoiser::Oidn, 64));
    assert_eq!(
        BakeSettings::default().samples,
        BakeSettings::DEFAULT_SAMPLES
    );
    for d in Denoiser::ALL {
        assert_eq!(Denoiser::from_name(&d.name().to_lowercase()), Some(d));
    }
    assert_eq!(Denoiser::from_name("OptiX"), None);
}
