use super::change::{dark_room, lamp};
use super::measure::run;
use super::room::{anti_flicker, renderer, DEFAULT, ON, WARM};
use crate::Renderer;
use rusty_components::MaterialAsset;
use rusty_scene::Scene;

const AT: u32 = 4;
const FLASHES: u32 = 5;

fn lifts(run: &[Vec<f32>], tile: usize, period: u32) -> Vec<f32> {
    let at = |k: u32| (AT + k * period) as usize;
    (0..FLASHES)
        .map(|k| run[at(k)][tile] - run[at(k) - 1][tile])
        .collect()
}

fn flashes(period: u32) -> (u32, impl Fn(&mut Scene, u32)) {
    let frames = AT + FLASHES * period + 1;
    let flash = move |s: &mut Scene, f: u32| {
        let on = f >= WARM + AT && (f - WARM - AT).is_multiple_of(period) && f < WARM + frames - 1;
        lamp(s, "Flash", on.then_some(30.0));
    };
    (frames, flash)
}

fn urp_flashes(r: &mut Renderer, period: u32) -> (Vec<Vec<f32>>, usize) {
    let (frames, flash) = flashes(period);
    let urp = run(r, &mut dark_room(), (frames, 1.0), &flash);
    let first = |t: usize| urp[AT as usize][t] - urp[AT as usize - 1][t];
    let tile = (0..urp[0].len())
        .max_by(|&a, &b| first(a).total_cmp(&first(b)))
        .unwrap();
    (urp, tile)
}

fn survival(
    r: &mut Renderer,
    period: u32,
    value: f32,
    (urp, tile): &(Vec<Vec<f32>>, usize),
) -> Vec<f32> {
    let (frames, flash) = flashes(period);
    let held = run(
        r,
        &mut anti_flicker(dark_room(), value),
        (frames, 1.0),
        &flash,
    );
    let (h, u) = (lifts(&held, *tile, period), lifts(urp, *tile, period));
    assert!(u[0] > 20.0, "the flash lit the ceiling: {:.1}", u[0]);
    h.iter().zip(&u).map(|(h, u)| h / u).collect()
}

const FIRST: f32 = 0.95;
/// Measured on lavapipe, flashes 4 and 5: at 1, every 6 and 10 frames, 9 % and 25 %; at 0.5,
/// 12 % and 58 %.
const HELD: [(f32, u32, f32); 4] = [
    (DEFAULT, 6, 0.12),
    (DEFAULT, 10, 0.58),
    (ON, 6, 0.09),
    (ON, 10, 0.25),
];
const BAND: f32 = 0.1;

#[test]
fn gpu_a_repeated_flash_is_held_from_its_third_time_under_anti_flicker() {
    let Some(mut r) = renderer() else { return };
    let mut urp = std::collections::BTreeMap::new();
    for (value, period, held) in HELD {
        let base = urp
            .entry(period)
            .or_insert_with(|| urp_flashes(&mut r, period));
        let kept = survival(&mut r, period, value, base);
        let shown: Vec<String> = kept.iter().map(|k| format!("{:.0}%", 100.0 * k)).collect();
        eprintln!(
            "[taa] at {value}, a flash every {period} frames, kept flash by flash: {}",
            shown.join(" ")
        );
        let at = format!("at {value}, every {period}");
        assert!(kept[..2].iter().all(|&k| k >= FIRST), "{at}: {shown:?}");
        let later = kept[3..].iter().all(|&k| (k - held).abs() <= BAND);
        assert!(later, "{at}, from the fourth: {shown:?}, not ~{held}");
    }
}

pub(super) fn noise_room() -> Scene {
    let mut s = dark_room();
    let noise = super::super::wall::noise([0.8; 3], [24.0, 24.0]);
    s.materials.insert(
        "Ceiling".into(),
        MaterialAsset {
            roughness: 0.8,
            ..noise
        },
    );
    lamp(&mut s, "Lamp", Some(4.0));
    s
}

fn afterglow(r: &mut Renderer, scale: f32, value: f32) -> Vec<f32> {
    let frames = AT + 1 + AFTERGLOW as u32 + 1;
    let flash = |s: &mut Scene, f: u32| lamp(s, "Flash", (f == WARM + AT).then_some(30.0));
    let none = |_: &mut Scene, _: u32| {};
    let room = || anti_flicker(noise_room(), value);
    let lit = run(r, &mut room(), (frames, scale), &flash);
    let dark = run(r, &mut room(), (frames, scale), &none);
    let off = |f: usize| lit[f].iter().zip(&dark[f]).map(|(a, b)| (a - b).abs());
    (AT as usize + 1..frames as usize)
        .map(|f| off(f).fold(0.0, f32::max))
        .collect()
}

/// Measured on lavapipe at render scale 1 and 0.67: 32 and 39 frames.
const AFTERGLOW: usize = 44;

fn lone_flash(scale: f32) {
    let Some(mut r) = renderer() else { return };
    let urp = afterglow(&mut r, scale, 0.0);
    for value in [DEFAULT, ON] {
        let held = afterglow(&mut r, scale, value);
        let over: Vec<f32> = held.iter().zip(&urp).map(|(h, u)| h - u).collect();
        let lasts = over.iter().rposition(|&o| o > 1.0).map_or(0, |i| i + 1);
        eprintln!(
            "[taa] scale {scale}, at {value}: a lone flash's end on a blinking ceiling, the frame after: \
             {:.1} held, {:.1} URP's; held over URP's by a level for {lasts} frames",
            held[0], urp[0]
        );
        assert!(lasts <= AFTERGLOW, "at scale {scale}, {value}: {over:?}");
    }
}

#[test]
fn gpu_a_lone_flash_on_a_blinking_ceiling_under_anti_flicker() {
    lone_flash(1.0);
}

#[test]
fn gpu_a_lone_flash_on_a_blinking_ceiling_under_anti_flicker_upscaled() {
    lone_flash(0.67);
}
