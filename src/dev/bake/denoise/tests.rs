//! OIDN's binding (#1884): where it looks, what a bake without it says, and, on a
//! machine that has it, that it cleans a noisy lightmap and gives the same bytes
//! every time, a whole bake's too, whatever order its workers finish the charts in.
//! The tests that run OIDN skip without it, except where
//! [`REQUIRE_OIDN_ENV`] is set (CI), whose canary then fails.

use std::path::{Path, PathBuf};

use glam::Vec3;

use super::{available, ffi, missing, Oidn, REQUIRE_OIDN_ENV};
use crate::scene::lighting::lightmap::{
    bake_with_progress, BakeLight, BakeProgress, BakeScene, BakeSettings, LightShape,
    LightmapDenoiser,
};
use crate::scene::lighting::occlusion::bench_level::rooms;

/// OIDN's denoiser, or `None` (skip) on a machine without it.
fn oidn_or_skip() -> Option<Oidn> {
    available().ok().map(|()| Oidn::default())
}

#[test]
fn oidn_is_present_where_ci_requires_it() {
    if std::env::var_os(REQUIRE_OIDN_ENV).is_some() {
        available().expect("RUSTY_REQUIRE_OIDN is set but OIDN does not open");
    }
}

#[test]
fn a_named_folder_is_searched_before_the_loader_and_homebrew() {
    let dir = crate::test_temp::dir();
    let found = ffi::candidates(Some(&dir));
    assert!(found[0].starts_with(&dir), "{found:?}");
    assert!(found.iter().any(|p| p.starts_with(dir.join("lib"))));
    let file = Path::new("/nowhere/libOpenImageDenoise.so.2");
    assert_eq!(ffi::candidates(Some(file))[0], file);
    let bare = ffi::candidates(None);
    assert_eq!(
        bare[0].parent(),
        Some(Path::new("")),
        "the loader's search first"
    );
    assert!(bare.iter().any(|p| p.starts_with("/opt/homebrew/lib")));
}

#[test]
fn a_missing_library_is_an_error_not_a_crash() {
    let nowhere = [PathBuf::from("/nowhere/libOpenImageDenoise.so.2")];
    let err = ffi::Api::open(&nowhere).err().expect("nothing there");
    assert!(err.contains("not found"), "{err}");
}

#[test]
fn the_missing_warning_says_what_to_install() {
    let text = missing("Open Image Denoise not found", 32);
    let needles = [
        "brew install open-image-denoise",
        "openimagedenoise",
        "RUSTY_OIDN",
        "32",
    ];
    for needle in needles {
        assert!(text.contains(needle), "{needle} in {text}");
    }
}

/// A 32² chart of flat light 1.0 with seeded ±0.5 grain.
fn noisy() -> Vec<Vec3> {
    let mut state = 0x2545_f491_u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32 - 0.5
    };
    (0..32 * 32).map(|_| Vec3::splat(1.0 + next())).collect()
}

/// Mean distance from the flat 1.0 the grain sits on.
fn grain(image: &[Vec3]) -> f32 {
    image.iter().map(|v| (v.x - 1.0).abs()).sum::<f32>() / image.len() as f32
}

#[test]
fn oidn_cleans_grain_and_repeats_its_bytes() {
    let Some(oidn) = oidn_or_skip() else { return };
    let mut once = noisy();
    oidn.denoise(&mut once, 32, false).expect("denoised");
    let (after, before) = (grain(&once), grain(&noisy()));
    assert!(after < before * 0.3, "{after} vs {before}");
    let mut twice = noisy();
    oidn.denoise(&mut twice, 32, false).expect("denoised");
    assert_eq!(once, twice, "same input, same bytes");
    assert_eq!(oidn.spent().0, 2, "two charts counted");
}

#[test]
fn oidn_refuses_an_image_of_the_wrong_size() {
    let Some(oidn) = oidn_or_skip() else { return };
    let mut short = vec![Vec3::ONE; 10];
    assert!(oidn.denoise(&mut short, 32, false).is_err());
    assert_eq!(oidn.spent().0, 0);
}

#[test]
fn a_whole_oidn_bake_repeats_its_bytes_whichever_worker_finishes_first() {
    let Some(_) = oidn_or_skip() else { return };
    let mut scene = BakeScene::gather(&rooms(1), &|_| None);
    let position = Vec3::new(3.0, 3.0, 3.0);
    let shape = LightShape::Point {
        position,
        range: 10.0,
    };
    let (radiance, bakes_direct, mask_channel) = (Vec3::splat(3.0), true, None);
    scene.lights.push(BakeLight {
        shape,
        radiance,
        bakes_direct,
        mask_channel,
    });
    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 4,
        ..Default::default()
    };
    let bake = || {
        let oidn = std::sync::Arc::new(Oidn::default());
        let progress = BakeProgress::default().with_denoiser(oidn.clone());
        let maps = bake_with_progress(&scene, &settings, &progress).expect("not cancelled");
        assert!(oidn.spent().0 > 0, "OIDN cleaned the charts");
        maps
    };
    assert_eq!(bake(), bake(), "the same bytes, colour and directions");
}
