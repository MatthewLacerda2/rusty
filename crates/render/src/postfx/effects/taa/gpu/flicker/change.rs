use glam::Vec3;

use super::measure::run;
use super::room::{anti_flicker, block, matte, renderer, room, ON, WARM};
use rusty_scene::create::default_light;
use rusty_scene::Scene;

const FRAMES: u32 = 16;
const AT: u32 = 6;
const CALM: f32 = 0.5;
const SAME: f32 = 1.0;

pub(super) fn apart(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
    let pairs = a
        .iter()
        .zip(b)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()));
    pairs.fold(0.0, f32::max)
}

pub(super) fn dark_room() -> Scene {
    let mut s = room();
    let bulb = s.find_entity_by_name("Bulb").unwrap();
    s.world.transform_mut(bulb).unwrap().position.y = -5.0;
    let cam = s.find_entity_by_name("Camera").unwrap();
    s.world.post_processing_mut(cam).unwrap().ssr.active = false;
    s
}

pub(super) fn lamp(s: &mut Scene, name: &str, intensity: Option<f32>) {
    let id = s.find_entity_by_name(name).unwrap_or_else(|| {
        let id = s.add_entity(name.to_string());
        s.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 2.4, -4.0);
        id
    });
    let light = intensity.map(|intensity| rusty_components::LightComponent {
        intensity,
        mode: rusty_components::LightMode::Mixed,
        ..default_light()
    });
    s.world.set_light(id, light);
}

#[test]
fn gpu_a_lone_flash_on_a_still_ceiling_is_clamped_as_urps() {
    let Some(mut r) = renderer() else { return };
    let flash = |s: &mut Scene, f: u32| lamp(s, "Flash", (f == WARM + AT).then_some(30.0));
    let held = run(
        &mut r,
        &mut anti_flicker(dark_room(), ON),
        (FRAMES, 1.0),
        &flash,
    );
    let urp = run(&mut r, &mut dark_room(), (FRAMES, 1.0), &flash);
    let lit = held[AT as usize]
        .iter()
        .zip(&held[AT as usize - 1])
        .map(|(a, b)| a - b);
    let rise = lit.fold(0.0, f32::max);
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

fn slide(s: &mut Scene, f: u32) {
    let cube = s.find_entity_by_name("Mover").unwrap_or_else(|| {
        let place = (Vec3::ZERO, Vec3::splat(0.3));
        block(s, "Mover", place, matte(0.5))
    });
    let x = -1.5 + 0.1 * f.saturating_sub(WARM) as f32;
    let y = if f < WARM + FRAMES - 4 { 2.4 } else { -5.0 };
    s.world.transform_mut(cube).unwrap().position = Vec3::new(x, y, -3.2);
}

const TRAIL: f32 = 1.0;

#[test]
fn gpu_a_cube_moving_under_a_flickering_ceiling_leaves_no_more_trail_than_urps() {
    let Some(mut r) = renderer() else { return };
    let none = &|_: &mut Scene, _: u32| {};
    let mut trail = |value| {
        let moved = run(
            &mut r,
            &mut anti_flicker(room(), value),
            (FRAMES, 1.0),
            &slide,
        );
        let empty = run(
            &mut r,
            &mut anti_flicker(room(), value),
            (FRAMES, 1.0),
            none,
        );
        let last = moved.len() - 1;
        apart(&moved[last..], &empty[last..])
    };
    let (held, urp) = (trail(ON), trail(0.0));
    eprintln!("[taa] the cube's trail, 4 frames after it left: {held:.1} held, {urp:.1} URP's");
    assert!(
        held <= urp + TRAIL,
        "the hold leaves a trail: {held:.1} against {urp:.1}"
    );
}
