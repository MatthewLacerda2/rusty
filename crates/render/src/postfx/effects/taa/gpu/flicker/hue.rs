use super::change::lamp;
use super::cost::noise_room;
use super::measure::run_by;
use super::room::{anti_flicker, renderer, ON, WARM};
use rusty_scene::Scene;

const FRAMES: u32 = 16;
const AT: u32 = 4;
/// Measured: lavapipe 0.4 %, RTX 2060 0.4 %; a hold of the whole colour, 89 %.
const SAME: f32 = 0.01;

fn co(ycocg: [f32; 3]) -> f32 {
    255.0 * ycocg[1] / ycocg[0].max(1e-3)
}

fn turn(s: &mut Scene, f: u32) {
    let blue = f >= WARM + AT;
    let id = s.find_entity_by_name("Lamp").unwrap();
    let mut light = s.world.light_mut(id).unwrap();
    light.color = if blue {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    }
    .into();
}

#[test]
fn gpu_a_hue_change_at_equal_luma_is_clamped_as_urps_under_anti_flicker() {
    let Some(mut r) = renderer() else { return };
    let mut lit = |value: f32| {
        let mut s = anti_flicker(noise_room(), value);
        lamp(&mut s, "Lamp", Some(12.0));
        run_by(&mut r, &mut s, (FRAMES, 1.0), &turn, co)
    };
    let (held, urp) = (lit(ON), lit(0.0));
    let before = &urp[AT as usize - 1];
    let swung = urp[AT as usize]
        .iter()
        .zip(before)
        .map(|(a, b)| (a - b).abs());
    let swung = swung.fold(0.0, f32::max);
    let gap = held[AT as usize..]
        .iter()
        .zip(&urp[AT as usize..])
        .map(|(h, u)| {
            h.iter()
                .zip(u)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max)
        });
    let gap = gap.fold(0.0, f32::max);
    eprintln!(
        "[taa] the lamp's turn moves a tile's hue {swung:.1}; held and URP's differ by {gap:.2}"
    );
    assert!(
        swung > 10.0,
        "the lamp's hue reached the ceiling: {swung:.1}"
    );
    assert!(
        gap <= SAME * swung,
        "the anti-flicker held the old hue: {gap:.2} off URP's"
    );
}
