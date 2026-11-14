//! The progressive bake (#1970): its last pass gives the one-shot bake's bytes
//! whatever its pass size, view or thread count, and a cancel returns nothing.
//! What each pass shows: `pass_tests`; Prioritize View:
//! `view_tests`.

use glam::Vec3;

use super::bake::bake_on;
use super::bake::progressive::progressive_on;
use super::tests::{floor, settings, sun, wall};
use super::*;
use rusty_scene::lighting::settings::BakeSettings;

/// The floor and wall under a `Baked` sun, a `Baked` Area panel (its direct light
/// drawn from the texel's stream after its paths) and a `Mixed` point lamp on
/// shadowmask channel 0, with a sky.
pub(super) fn lit() -> BakeScene {
    let area = AreaShape {
        center: Vec3::new(0.0, 2.0, 0.0),
        right: Vec3::X,
        up: Vec3::Z * 0.5,
        forward: Vec3::NEG_Y,
        range: 8.0,
    };
    let lamp = BakeLight {
        shape: LightShape::Point {
            position: Vec3::new(-1.0, 1.0, 1.0),
            range: 6.0,
        },
        radiance: Vec3::splat(2.0),
        bakes_direct: false,
        mask_channel: Some(0),
    };
    let panel = BakeLight {
        shape: LightShape::Area(area),
        radiance: Vec3::splat(2.0),
        bakes_direct: true,
        mask_channel: None,
    };
    let mut shadowmask = MaskChannels::default();
    shadowmask.channels.insert(9, 0);
    BakeScene {
        shadowmask,
        meshes: vec![floor(), wall()],
        lights: vec![sun(true), panel, lamp],
        sky: Vec3::new(0.3, 0.4, 0.5).into(),
        diffuse: Default::default(),
    }
}

/// Every float of `maps`, as bits.
pub(super) fn bits(maps: &[Lightmap]) -> Vec<u32> {
    let colour = maps
        .iter()
        .flat_map(|m| m.texels.iter().map(|t| t.to_array()));
    let rest = maps
        .iter()
        .flat_map(|m| m.directions.iter().chain(&m.masks));
    let colour = colour.flat_map(|t| t.map(f32::to_bits));
    colour
        .chain(rest.flat_map(|d| d.to_array().map(f32::to_bits)))
        .collect()
}

/// `scene` baked progressively at `s` on `threads`, and each pass it reported.
pub(super) fn progressive(
    scene: &BakeScene,
    s: &BakeSettings,
    view: Option<BakeView>,
    threads: usize,
) -> (Vec<Lightmap>, Vec<(Pass, usize)>) {
    let mut passes = Vec::new();
    let mut heard = |pass: Pass, maps: Vec<Lightmap>| passes.push((pass, maps.len()));
    let progress = BakeProgress::default();
    let maps = progressive_on(scene, s, &progress, (view, threads), &mut heard);
    (maps.expect("not cancelled"), passes)
}

/// A camera at `eye` looking at `at`, `fov` radians high.
pub(super) fn camera(eye: Vec3, at: Vec3, fov: f32) -> BakeView {
    let proj = glam::camera::rh::proj::directx::perspective(fov, 1.0, 0.05, 50.0);
    BakeView {
        eye,
        view_proj: proj * glam::camera::rh::view::look_at_mat4(eye, at, Vec3::Y),
    }
}
#[test]
fn the_last_pass_gives_the_one_shot_bytes() {
    let scene = lit();
    // 10 samples in passes of 4: 4, 4, then the 2 left.
    let s = BakeSettings {
        samples: 10,
        samples_per_pass: 4,
        filter_radius: 2,
        ..settings()
    };
    let one_shot = bake_on(&scene, &s, &BakeProgress::default(), 1).expect("not cancelled");
    assert!(one_shot
        .iter()
        .all(|m| !m.masks.is_empty() && !m.directions.is_empty()));
    let view = camera(Vec3::new(-1.0, 3.0, 4.0), Vec3::ZERO, 1.0);
    for (threads, view) in [(1, None), (3, None), (1, Some(view)), (4, Some(view))] {
        let (maps, passes) = progressive(&scene, &s, view, threads);
        assert_eq!(bits(&maps), bits(&one_shot), "{threads} threads, {view:?}");
        let heard: Vec<u32> = passes.iter().map(|(p, _)| p.samples).collect();
        assert_eq!(heard, [4, 8], "every pass but the last is shown");
        assert!(passes.iter().all(|(p, n)| p.passes == 3 && *n == 2));
    }
    // A pass as large as the bake is the one-shot bake, with nothing shown.
    let whole = BakeSettings {
        samples_per_pass: 64,
        ..s
    };
    let (maps, passes) = progressive(&scene, &whole, None, 2);
    assert_eq!((bits(&maps), passes.len()), (bits(&one_shot), 0));
}
#[test]
fn a_cancelled_progressive_bake_returns_nothing() {
    let progress = BakeProgress::default();
    progress.cancel();
    let s = BakeSettings {
        samples_per_pass: 4,
        ..settings()
    };
    let mut heard = 0;
    let out = progressive_on(&lit(), &s, &progress, (None, 2), &mut |_, _| heard += 1);
    assert!(out.is_none() && heard == 0);
}

#[test]
fn passes_cover_the_samples_the_last_taking_the_rest() {
    let passes = |samples, samples_per_pass| {
        let s = BakeSettings {
            samples,
            samples_per_pass,
            ..settings()
        };
        s.sanitized().passes()
    };
    assert_eq!(
        (passes(64, 8), passes(10, 4), passes(4, 64), passes(5, 0)),
        (8, 3, 1, 5)
    );
}
