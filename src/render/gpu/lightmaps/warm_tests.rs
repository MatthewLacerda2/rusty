//! A warm-up and the frames of the scene still drawing share one array cache, with
//! pages read on a worker as a windowed renderer reads them (#2123): neither takes
//! the other's landing, and each counts only what it uploaded.

use std::time::Duration;

use super::Lightmaps;
use crate::render::stats::Uploads;
use crate::render::test_gpu::headless_or_skip;

/// A black `edge`² PNG named `name` under the test's temp dir, as a one-page list.
fn list(name: &str, edge: u32) -> Vec<String> {
    let dir = crate::test_temp::dir().join("rusty_2123_lightmaps");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    image::RgbaImage::new(edge, edge).save(&path).unwrap();
    vec![path.to_string_lossy().into_owned()]
}

/// Call `f` until it answers `true`, sleeping between tries.
fn until(mut f: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("the page worker never answered");
}

#[test]
fn gpu_a_list_the_warm_up_lands_is_bound_by_the_next_frame() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let mut maps = Lightmaps::new(device, false);
    let level = list("retry.png", 2);
    // A retry of the level whose pages are still being read: the warm-up lands them.
    maps.bind(device, queue, &level, &[], &[]);
    until(|| maps.warm(device, queue, [&level, &[], &[]]));
    assert!(maps.bind(device, queue, &level, &[], &[]), "rebinds");
    assert!(maps.resident(), "the level is lit");
    assert!(!maps.bind(device, queue, &level, &[], &[]), "and only once");
}

#[test]
fn gpu_a_warm_up_and_the_drawing_scene_never_take_each_others_landing() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let mut maps = Lightmaps::new(device, false);
    let (splash, next) = (list("splash.png", 2), list("next.png", 4));
    maps.bind(device, queue, &splash, &[], &[]);
    // The splash's `Start` loads the next level: its warm-up runs beside its frames.
    let mut warmed = Uploads::default();
    let mut framed = Uploads::default();
    until(|| {
        let ready = maps.warm(device, queue, [&next, &[], &[]]);
        warmed.add_all(std::mem::take(&mut maps.uploads));
        maps.bind(device, queue, &splash, &[], &[]);
        framed.add_all(std::mem::take(&mut maps.uploads));
        ready
    });
    until(|| {
        maps.bind(device, queue, &splash, &[], &[]);
        framed.add_all(std::mem::take(&mut maps.uploads));
        maps.resident()
    });
    assert_eq!(
        maps.bound_edge(),
        Some(2),
        "the splash is lit by its own pages"
    );
    assert_eq!((warmed.count, framed.count), (1, 1), "each counts its own");
    assert!(
        maps.bind(device, queue, &next, &[], &[]),
        "the level swaps in"
    );
    assert_eq!(maps.bound_edge(), Some(4), "warm: bound on its first frame");
    assert_eq!(
        std::mem::take(&mut maps.uploads).count,
        0,
        "uploading nothing"
    );
}
