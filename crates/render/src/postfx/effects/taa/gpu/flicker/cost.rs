//! What the anti-flicker (`super`) costs, the numbers `docs/api/Graphics.md`'s
//! *Anti-Flicker* tells a game: a one-frame flash repeated on a calm still ceiling
//! every 6 and every 10 frames, flash by flash against URP's; and a lone flash on a
//! ceiling of noise that blinks under the jitter, how long its end stays, at
//! render scale 1 and at 0.67 (TAA upscaling), one test each. URP's run of a case
//! is shared by both anti-flicker values it is compared at (#2350).

use super::change::{dark_room, lamp};
use super::measure::run;
use super::room::{anti_flicker, renderer, DEFAULT, ON, WARM};
use crate::Renderer;
use rusty_components::MaterialAsset;
use rusty_scene::Scene;

/// The first flash's frame after [`WARM`], and how many come: the first two, the
/// third (where the hold starts) and two more, which the hold keeps alike (a sixth
/// read the same to the percent on lavapipe and an RTX 2060, and cost a period of
/// frames in every run, #2350).
const AT: u32 = 4;
const FLASHES: u32 = 5;

/// Each flash's lift of `tile` in `run` (its frame over the frame before), flashes
/// `period` apart from [`AT`].
fn lifts(run: &[Vec<f32>], tile: usize, period: u32) -> Vec<f32> {
    let at = |k: u32| (AT + k * period) as usize;
    (0..FLASHES)
        .map(|k| run[at(k)][tile] - run[at(k) - 1][tile])
        .collect()
}

/// The flashes every `period` frames from [`AT`], and the run's length.
fn flashes(period: u32) -> (u32, impl Fn(&mut Scene, u32)) {
    let frames = AT + FLASHES * period + 1;
    let flash = move |s: &mut Scene, f: u32| {
        let on = f >= WARM + AT && (f - WARM - AT).is_multiple_of(period) && f < WARM + frames - 1;
        lamp(s, "Flash", on.then_some(30.0));
    };
    (frames, flash)
}

/// URP's run of flashes every `period` frames, and the tile its first flash lit most.
fn urp_flashes(r: &mut Renderer, period: u32) -> (Vec<Vec<f32>>, usize) {
    let (frames, flash) = flashes(period);
    let urp = run(r, &mut dark_room(), (frames, 1.0), &flash);
    let first = |t: usize| urp[AT as usize][t] - urp[AT as usize - 1][t];
    let tile = (0..urp[0].len())
        .max_by(|&a, &b| first(a).total_cmp(&first(b)))
        .unwrap();
    (urp, tile)
}

/// What of each flash survives the anti-flicker at `value`, against URP's run
/// (`urp_flashes`, shared by both values of a period), on its brightest tile.
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

/// How much of a repeated flash the anti-flicker keeps, against URP's: the first
/// two flashes at least [`FIRST`] (no hold before a blink recurs); from the fourth
/// on, flashes `period` frames apart under `value` keep [`HELD`]'s share, within
/// [`BAND`]. Measured on lavapipe, flash by flash: at 1, every 6 frames 100 % 100 %
/// 10 % 9 % 9 % 9 %, every 10 100 % 100 % 40 % 25 % 25 % 25 % (RTX 2060: the same,
/// to the percent); at 0.5 (the default), every 6 100 % 100 % 30 % 12 % 12 % 13 %,
/// every 10 100 % 100 % 70 % 59 % 58 % 58 % (RTX 2060: the same, to the percent).
const FIRST: f32 = 0.95;
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

/// The dark room's ceiling a grey noise, lit by a steady lamp: a calm still view
/// whose texels blink under the jitter, as a textured level's do.
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

/// How long a lone flash's end stays on the noise ceiling at render `scale`: per
/// frame after the flash, the most any tile still sits off the flash-free run at
/// anti-flicker `value` (0 is URP's). It reads one frame past [`AFTERGLOW`], so an
/// end held longer still shows as over it.
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

/// How many frames after a lone flash the noise ceiling's held tiles may still sit
/// a level over URP's (`Graphics.md`): ours, the measure plus a margin. Measured
/// on lavapipe: 32 frames at render scale 1 (29 levels the frame after, against
/// URP's 9), 39 at 0.67 (20 against 9); on an RTX 2060, 30 (25 against 8) and
/// 38 (20 against 9). At 0.5 (the default) less: lavapipe 29 frames (23 against 9)
/// and 35 (18 against 9), RTX 2060 26 (20 against 8) and 34 (16 against 9); the one
/// bound holds both.
const AFTERGLOW: usize = 44;

/// The lone flash at render `scale`, held at the default and at full strength,
/// each against URP's (one run of it for both).
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

// One test per scale, so the two halves run side by side in the `gpu` group.
#[test]
fn gpu_a_lone_flash_on_a_blinking_ceiling_under_anti_flicker() {
    lone_flash(1.0);
}

#[test]
fn gpu_a_lone_flash_on_a_blinking_ceiling_under_anti_flicker_upscaled() {
    lone_flash(0.67);
}
