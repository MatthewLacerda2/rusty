//! Progress and cancel (#808): watching a bake never changes its bytes, the counter
//! ends at the total, and a cancelled bake returns nothing.

use glam::Vec3;

use super::tests::{floor, settings, sun, wall};
use super::*;

fn scene() -> BakeScene {
    BakeScene {
        meshes: vec![floor(), wall()],
        lights: vec![sun(true)],
        sky: Vec3::new(0.3, 0.4, 0.5),
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

#[test]
fn progress_counts_every_texel_up_to_the_total() {
    let progress = BakeProgress::default();
    assert_eq!((progress.done(), progress.total()), (0, 0));
    bake_with_progress(&scene(), &settings(), &progress).expect("not cancelled");
    assert!(progress.total() > 0);
    assert_eq!(progress.done(), progress.total());
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
