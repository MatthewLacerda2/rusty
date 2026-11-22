use glam::Vec3;

use super::fixture::H;
use super::fixture::{
    frame, linear, pixel, renderer, rgb, tuned, Surface, MIRROR, RED, REFLECTING,
};
use crate::ibl::ktx2_encode::encode_cubemap;
use crate::ibl::reflection_prefilter::source::encode_face;
use crate::ibl::reflection_prefilter::PrefilteredCubemap;
use rusty_scene::{FogMode, Scene};

const BLACK: Surface = ([0.0; 3], 0.0, 1.0);

fn glowing(on: bool, floor: Surface, glow: [f32; 3], ssao: bool) -> Scene {
    let (mut scene, _) = tuned(on, floor, BLACK, |v| {
        (v.ssao.active, v.ssao.radius, v.ssao.intensity) = (ssao, 1.0, 4.0);
        v.tonemapping.mode = rusty_components::Tonemap::None;
    });
    scene.materials.get_mut("cube").unwrap().emissive = glow;
    scene.ambient_color = Vec3::splat(0.2);
    scene
}

fn green_kept(r: &mut crate::Renderer, z: f32, ssao: bool) -> f32 {
    let scene = |on| glowing(on, MIRROR, [1.0, 0.02, 0.02], ssao);
    let (on, off) = (frame(r, &scene(true)), frame(r, &scene(false)));
    let (x, y) = pixel(Vec3::new(0.0, 0.0, z));
    let patch: Vec<_> = (y.saturating_sub(1)..(y + 2).min(H))
        .flat_map(|y| (x - 2..=x + 2).map(move |x| (x, y)))
        .collect();
    let green = |f: &[u8]| patch.iter().map(|&p| linear(f, p)[1]).sum::<f32>();
    green(&on) / green(&off)
}

#[test]
fn gpu_ssao_occludes_the_reflection_ssr_swaps_in_as_it_did_the_sky() {
    let Some(mut r) = renderer() else { return };
    // 0.4 m off the cube: the march hits solidly there (Metal missed the 0.1 m crease).
    let kept = |r: &mut _, ssao| green_kept(r, -5.1, ssao);
    let (open, occluded) = (kept(&mut r, false), kept(&mut r, true));
    eprintln!("[ssr] the floor keeps {open} of its green open, {occluded} under SSAO");
    assert!(open < 0.3, "SSR shows the red cube there");
    assert!(
        (occluded - open).abs() <= open * 0.25,
        "SSAO scales what SSR swaps"
    );
}

#[test]
fn gpu_a_gold_mirrors_hit_is_tinted_by_its_f0() {
    let Some(mut r) = renderer() else { return };
    let gold = ([1.0, 0.77, 0.34], 1.0, 0.0);
    let at = pixel(REFLECTING);
    let on = linear(&frame(&mut r, &glowing(true, gold, [1.0; 3], false)), at);
    let off = linear(&frame(&mut r, &glowing(false, gold, [1.0; 3], false)), at);
    let gained = [0, 1, 2].map(|c| on[c] - off[c]);
    eprintln!("[ssr] gold: on {on:?}, off {off:?}, gained {gained:?}");
    assert!(gained[0] > 0.1, "the hit brightens the gold");
    assert!(gained[2] < gained[0] * 0.7, "the hit is tinted by F0");
}

const PROBE_LINEAR: f32 = 0.2158;

fn white_cube() -> String {
    let face = encode_face(&[Vec3::splat(PROBE_LINEAR); 4 * 4]);
    let cube = PrefilteredCubemap {
        mips: vec![std::array::from_fn(|_| face.clone())],
        base_size: 4,
    };
    let path = rusty_test_support::temp::dir().join("ssr_grey_probe.ktx2");
    rusty_core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn gpu_a_hit_replaces_a_probes_reflection_instead_of_adding_to_it() {
    let Some(mut r) = renderer() else { return };
    let cube = white_cube();
    let probed = |on| {
        let mut scene = glowing(on, MIRROR, [PROBE_LINEAR; 3], false);
        let set = &mut scene.reflection_probes;
        let i = set.add(Vec3::new(0.0, 0.0, -10.0), Vec3::splat(40.0));
        set.set_cubemap(i, cube.clone());
        scene
    };
    let at = pixel(REFLECTING);
    let on = linear(&frame(&mut r, &probed(true)), at);
    let off = linear(&frame(&mut r, &probed(false)), at);
    eprintln!("[ssr] probe: on {on:?}, off {off:?}");
    assert!(off[1] > 0.1, "the probe lights the mirror");
    assert!(
        (0..3).all(|c| (on[c] - off[c]).abs() <= off[c] * 0.1),
        "no double reflection"
    );
}

#[test]
fn gpu_a_fogged_floor_keeps_its_fog() {
    let Some(mut r) = renderer() else { return };
    let fogged = |on, fog: bool| {
        let (mut scene, _) = tuned(on, MIRROR, RED, |_| {});
        if fog {
            let f = &mut scene.fog;
            (f.mode, f.start, f.end, f.color) =
                (FogMode::Linear, 1.0, 4.0, Vec3::new(0.8, 0.1, 0.1));
        }
        scene
    };
    let at = pixel(REFLECTING);
    let change = |r: &mut _, fog| {
        let on = rgb(&frame(r, &fogged(true, fog)), at);
        let off = rgb(&frame(r, &fogged(false, fog)), at);
        (0..3).map(|c| (on[c] - off[c]).abs()).max().unwrap()
    };
    let (clear, fog) = (change(&mut r, false), change(&mut r, true));
    eprintln!("[ssr] fog: SSR changes the floor by {clear} clear, {fog} fogged");
    assert!(clear >= 40, "SSR shows the cube in the clear");
    assert!(fog <= 2, "a floor all fog shows no reflection");
}
