//! What the hold (`super`) must leave alone: a lone flash on a still wall is
//! clamped as URP's is, on and off, and a cube moving under the flickering
//! ceiling leaves no more trail than under URP's resolve.

use glam::Vec3;

use super::super::super::params::Rules;
use super::room::{block, matte, renderer, room, run, WARM};
use rusty_scene::create::default_light;
use rusty_scene::Scene;

/// Frames measured, and the one (after [`WARM`]) a change comes on.
const FRAMES: u32 = 16;
const AT: u32 = 6;
/// A tile's largest step before the flash, in 255ths, for it to count as calm.
const CALM: f32 = 0.5;
/// How far a calm tile under the hold may sit from URP's through a lone flash, in
/// 255ths: a level. Lavapipe measures 0.5, a calm pixel's own sub-level blinks
/// (lighting's rounding) holding that little of the flash's end.
const SAME: f32 = 1.0;
/// URP's resolve: the hold off.
const URP: Rules = Rules {
    moving: true,
    reveal: true,
    flicker: false,
};

/// The largest difference between two runs' tiles, over every frame.
fn apart(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
    let pairs = a
        .iter()
        .zip(b)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()));
    pairs.fold(0.0, f32::max)
}

/// The room with its bulb hidden under the floor and SSR off, so nothing in it
/// blinks: a pixel that blinks already holds a lone change's end as far as its
/// own flicker reaches (`params::FLICKER_HOLD`).
fn dark_room() -> Scene {
    let mut s = room();
    let bulb = s.find_entity_by_name("Bulb").unwrap();
    s.world.transform_mut(bulb).unwrap().position.y = -5.0;
    let cam = s.find_entity_by_name("Camera").unwrap();
    s.world.post_processing_mut(cam).unwrap().ssr.active = false;
    s
}

#[test]
fn gpu_a_lone_flash_on_a_still_ceiling_is_clamped_as_urps() {
    let Some(mut r) = renderer() else { return };
    let flash = |s: &mut Scene, f: u32| {
        let lamp = s.find_entity_by_name("Flash").unwrap_or_else(|| {
            let id = s.add_entity("Flash".to_string());
            s.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 2.4, -4.0);
            id
        });
        let light = rusty_components::LightComponent {
            intensity: 30.0,
            ..default_light()
        };
        s.world.set_light(lamp, (f == WARM + AT).then_some(light));
    };
    let held = run(&mut r, &mut dark_room(), (FRAMES, Rules::default()), &flash);
    let urp = run(&mut r, &mut dark_room(), (FRAMES, URP), &flash);
    let lit = held[AT as usize]
        .iter()
        .zip(&held[AT as usize - 1])
        .map(|(a, b)| a - b);
    let rise = lit.fold(0.0, f32::max);
    // The tiles nothing blinked in before the flash (an edge's aliasing does).
    let calm: Vec<usize> = (0..urp[0].len())
        .filter(|&t| {
            urp[..AT as usize]
                .windows(2)
                .all(|w| (w[0][t] - w[1][t]).abs() < CALM)
        })
        .collect();
    let pick = |run: &[Vec<f32>]| -> Vec<Vec<f32>> {
        run.iter()
            .map(|f| calm.iter().map(|&t| f[t]).collect())
            .collect()
    };
    let gap = apart(&pick(&held), &pick(&urp));
    eprintln!(
        "[taa] a lone flash lifts a tile {rise:.1}; over {} calm tiles of {}, held and URP's differ by {gap:.2}",
        calm.len(),
        urp[0].len()
    );
    assert!(rise > 20.0, "the flash lit the ceiling: {rise:.1}");
    assert!(calm.len() * 2 > urp[0].len(), "most of the room is calm");
    assert!(gap <= SAME, "a lone flash is held: {gap:.2} off URP's");
}

/// The cube's place on frame `f`: crossing under the bulb's reflection, then gone.
fn slide(s: &mut Scene, f: u32) {
    let cube = s.find_entity_by_name("Mover").unwrap_or_else(|| {
        let place = (Vec3::ZERO, Vec3::splat(0.3));
        block(s, "Mover", place, matte(0.5))
    });
    let x = -1.5 + 0.1 * f.saturating_sub(WARM) as f32;
    let y = if f < WARM + FRAMES - 4 { 2.4 } else { -5.0 };
    s.world.transform_mut(cube).unwrap().position = Vec3::new(x, y, -3.2);
}

#[test]
fn gpu_a_cube_moving_under_a_flickering_ceiling_leaves_no_more_trail_than_urps() {
    let Some(mut r) = renderer() else { return };
    let none = &|_: &mut Scene, _: u32| {};
    let mut trail = |rules| {
        let moved = run(&mut r, &mut room(), (FRAMES, rules), &slide);
        let empty = run(&mut r, &mut room(), (FRAMES, rules), none);
        let last = moved.len() - 1;
        apart(&moved[last..], &empty[last..])
    };
    let (held, urp) = (trail(Rules::default()), trail(URP));
    eprintln!("[taa] the cube's trail, 4 frames after it left: {held:.1} held, {urp:.1} URP's");
    assert!(
        held <= urp + 1.0,
        "the hold leaves a trail: {held:.1} against {urp:.1}"
    );
}
