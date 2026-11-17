//! What the anti-flicker (`super`) costs, the numbers `docs/api/Graphics.md`'s
//! *Anti-Flicker* tells a game: a one-frame flash repeated on a calm still ceiling
//! every 6 and every 10 frames, flash by flash against URP's; and a lone flash on a
//! ceiling of noise that blinks under the jitter, how long its end stays, at
//! render scale 1 and at 0.67 (TAA upscaling).

use super::change::{dark_room, lamp};
use super::measure::run;
use super::room::{anti_flicker, renderer, ON, WARM};
use crate::Renderer;
use rusty_components::MaterialAsset;
use rusty_scene::Scene;

/// The first flash's frame after [`WARM`], and how many come.
const AT: u32 = 4;
const FLASHES: u32 = 6;

/// Each flash's lift of `tile` in `run` (its frame over the frame before), flashes
/// `period` apart from [`AT`].
fn lifts(run: &[Vec<f32>], tile: usize, period: u32) -> Vec<f32> {
    let at = |k: u32| (AT + k * period) as usize;
    (0..FLASHES)
        .map(|k| run[at(k)][tile] - run[at(k) - 1][tile])
        .collect()
}

/// What of each flash survives the anti-flicker, against URP's, on the tile the
/// first flash lit most.
fn survival(r: &mut Renderer, period: u32, value: f32) -> Vec<f32> {
    let frames = AT + FLASHES * period + 1;
    let flash = |s: &mut Scene, f: u32| {
        let on = f >= WARM + AT && (f - WARM - AT).is_multiple_of(period) && f < WARM + frames - 1;
        lamp(s, "Flash", on.then_some(30.0));
    };
    let scene = &mut anti_flicker(dark_room(), value);
    let held = run(r, scene, (frames, 1.0), &flash);
    let urp = run(r, &mut dark_room(), (frames, 1.0), &flash);
    let first = |t: usize| urp[AT as usize][t] - urp[AT as usize - 1][t];
    let tile = (0..urp[0].len())
        .max_by(|&a, &b| first(a).total_cmp(&first(b)))
        .unwrap();
    let (h, u) = (lifts(&held, tile, period), lifts(&urp, tile, period));
    assert!(u[0] > 20.0, "the flash lit the ceiling: {:.1}", u[0]);
    h.iter().zip(&u).map(|(h, u)| h / u).collect()
}

/// How much of a repeated flash the anti-flicker keeps, against URP's: the first
/// two flashes at least [`FIRST`] (no hold before a blink recurs); from the fourth
/// on, flashes `period` frames apart under `value` keep [`HELD`]'s share, within
/// [`BAND`]. Measured on lavapipe, flash by flash: at 1, every 6 frames 100 % 100 %
/// 10 % 9 % 9 % 9 %, every 10 100 % 100 % 40 % 25 % 25 % 25 % (RTX 2060: the same,
/// to the percent); at 0.5 (the default), every 6 100 % 100 % 30 % 12 % 12 % 13 %,
/// every 10 100 % 100 % 70 % 59 % 58 % 58 %.
const FIRST: f32 = 0.95;
const HELD: [(f32, u32, f32); 4] = [
    (0.5, 6, 0.12),
    (0.5, 10, 0.58),
    (ON, 6, 0.09),
    (ON, 10, 0.25),
];
const BAND: f32 = 0.1;

#[test]
fn gpu_a_repeated_flash_is_held_from_its_third_time_under_anti_flicker() {
    let Some(mut r) = renderer() else { return };
    for (value, period, held) in HELD {
        let kept = survival(&mut r, period, value);
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
/// frame after the flash, the most any tile still sits off the flash-free run,
/// held and URP's.
fn afterglow(r: &mut Renderer, scale: f32) -> (Vec<f32>, Vec<f32>) {
    let frames = AT + 48;
    let flash = |s: &mut Scene, f: u32| lamp(s, "Flash", (f == WARM + AT).then_some(30.0));
    let none = |_: &mut Scene, _: u32| {};
    let mut glow = |value: f32| {
        let lit = run(
            r,
            &mut anti_flicker(noise_room(), value),
            (frames, scale),
            &flash,
        );
        let dark = run(
            r,
            &mut anti_flicker(noise_room(), value),
            (frames, scale),
            &none,
        );
        let off = |f: usize| lit[f].iter().zip(&dark[f]).map(|(a, b)| (a - b).abs());
        let after = AT as usize + 1..frames as usize;
        after
            .map(|f| off(f).fold(0.0, f32::max))
            .collect::<Vec<f32>>()
    };
    (glow(ON), glow(0.0))
}

/// How many frames after a lone flash the noise ceiling's held tiles may still sit
/// a level over URP's (`Graphics.md`): ours, the measure plus a margin. Measured
/// on lavapipe: 32 frames at render scale 1 (29 levels the frame after, against
/// URP's 9), 39 at 0.67 (20 against 9); on an RTX 2060, 30 (25 against 8) and
/// 38 (20 against 9). At 0.5 (the default) less: lavapipe 29 frames (23 against 9)
/// and 35 (18 against 9), so the bound is held at 1 alone.
const AFTERGLOW: usize = 44;

#[test]
fn gpu_a_lone_flash_on_a_blinking_ceiling_under_anti_flicker() {
    let Some(mut r) = renderer() else { return };
    for scale in [1.0, 0.67] {
        let (held, urp) = afterglow(&mut r, scale);
        let over: Vec<f32> = held.iter().zip(&urp).map(|(h, u)| h - u).collect();
        let lasts = over.iter().rposition(|&o| o > 1.0).map_or(0, |i| i + 1);
        eprintln!(
            "[taa] scale {scale}: a lone flash's end on a blinking ceiling, the frame after: \
             {:.1} held, {:.1} URP's; held over URP's by a level for {lasts} frames",
            held[0], urp[0]
        );
        assert!(lasts <= AFTERGLOW, "at scale {scale}: {over:?}");
    }
}
