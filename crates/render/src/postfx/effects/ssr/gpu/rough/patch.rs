use glam::Vec3;

use super::super::fixture::{paint, place};
use super::super::silhouette::{at, shot, BRONZE, W};
use super::edge::{lifted, side_step};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

fn tiled(on: bool, floor: f32, tile: f32, xs: (f32, f32), far: bool) -> Scene {
    let mut s = lifted(on, floor);
    if far {
        add_wall(&mut s);
    }
    let id = create_entity(&mut s, "Tile", Some(Primitive::Box));
    let at = Vec3::new((xs.0 + xs.1) / 2.0, -0.045, -3.5);
    place(&mut s, id, at, Vec3::new(xs.1 - xs.0, 0.1, 4.0));
    paint(&mut s, id, "tile", (BRONZE, 1.0, tile));
    s
}

fn add_wall(s: &mut Scene) {
    let wall = create_entity(s, "Wall", Some(Primitive::Box));
    let far = (Vec3::new(0.0, 2.0, -16.0), Vec3::new(30.0, 4.0, 0.2));
    place(s, wall, far.0, far.1);
    paint(s, wall, "wall", ([0.2, 0.4, 0.9], 0.0, 1.0));
}

fn step(r: &mut crate::Renderer, scene: impl Fn(bool) -> Scene) -> f32 {
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let (on, off) = (shot(r, &scene(true), &down), shot(r, &scene(false), &down));
    let red = |f: &[u8], x: u32, y: u32| at(f, x, y)[0] - at(f, x, y)[2];
    let added = |x: u32, y: u32| red(&on, x, y) - red(&off, x, y);
    let inside = W / 2 - 16;
    let row = (120..240).max_by_key(|&y| added(inside, y)).unwrap();
    let rise = (1..=inside).map(|x| added(x, row) - added(x - 1, row));
    rise.max().unwrap() as f32 / added(inside, row).max(1) as f32
}

const UNDER: (f32, f32) = (-2.0, -0.1);
const BESIDE: (f32, f32) = (-4.0, -0.7);

const KEPT: f32 = 0.9;

#[test]
fn gpu_a_smooth_patch_of_satin_metal_reflects_as_sharp_as_its_own_roughness() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(320, 240) else {
        return;
    };
    let smooth = step(&mut r, |on| tiled(on, 0.15, 0.15, UNDER, false));
    let patch = step(&mut r, |on| tiled(on, 0.35, 0.15, UNDER, false));
    eprintln!("[ssr] the lifted cube's reflected side steps {smooth:.2} on roughness 0.15, {patch:.2} on a 0.15 patch in 0.35");
    assert!(
        patch >= smooth * KEPT,
        "a smooth patch is blurred by its own roughness"
    );
}

#[test]
fn gpu_a_smooth_floor_beside_a_rough_one_keeps_its_own_blur() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(320, 240) else {
        return;
    };
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let mut sharp = |tile| {
        let scene = |on| tiled(on, 0.12, tile, BESIDE, true);
        side_step(
            &shot(&mut r, &scene(true), &down),
            &shot(&mut r, &scene(false), &down),
        )
        .0
    };
    let (alone, beside) = (sharp(0.12), sharp(0.6));
    eprintln!("[ssr] the lifted cube's reflected side on a roughness-0.12 floor steps {alone:.2}, {beside:.2} beside a 0.6 tile");
    assert!(
        beside >= alone * KEPT,
        "a smooth floor keeps its own blur beside a rough one"
    );
}
