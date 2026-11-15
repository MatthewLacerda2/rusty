//! A contact reflection keeps its own blur among long faded rays (#2158): a cube
//! standing on a satin metal floor, a far wall behind it. Most of the floor's rays
//! run to the wall and fade out (their blur too wide to keep), but they still tell
//! the resolve how wide a blur is around them; the rays that meet the cube's foot
//! are short, so its reflection there stays tight against the foot, not smeared
//! and dimmed across the long rays' blur.

use glam::Vec3;

use super::super::fixture::{paint, place};
use super::super::silhouette::{base, BRONZE};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

/// [`base`]'s cube standing on a roughness-0.35 floor, with or without a `wall`
/// 16 m out.
fn standing(on: bool, wall: bool) -> Scene {
    let mut s = base(on, (BRONZE, 1.0, 0.35));
    if !wall {
        return s;
    }
    let wall = create_entity(&mut s, "Wall", Some(Primitive::Box));
    place(
        &mut s,
        wall,
        Vec3::new(0.0, 2.0, -16.0),
        Vec3::new(30.0, 4.0, 0.2),
    );
    paint(&mut s, wall, "wall", ([0.2, 0.4, 0.9], 0.0, 1.0));
    s
}

/// The frame's size: the long rays' blur spans tens of pixels.
const W: u32 = 640;
const H: u32 = 480;

fn shot(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

/// Pixel `(x, y)`'s redness: red over blue.
fn red(f: &[u8], x: u32, y: u32) -> i32 {
    let i = ((y * W + x) * 4) as usize;
    i32::from(f[i]) - i32::from(f[i + 2])
}

/// The redness the reflection adds under the cube's foot: over its width, in
/// the rows down to a fortieth of the frame below it.
fn under_the_foot(on: &[u8], off: &[u8]) -> i32 {
    let cube = |y: u32| (0..W).filter(move |&x| red(off, x, y) > 60);
    let foot = (0..H).filter(|&y| cube(y).next().is_some()).max();
    let foot = foot.expect("the cube is in view");
    let (left, right) = (cube(foot).min().unwrap(), cube(foot).max().unwrap());
    let rows = foot + 2..foot + 2 + H / 40;
    rows.flat_map(|y| (left..=right).map(move |x| (x, y)))
        .map(|(x, y)| red(on, x, y) - red(off, x, y))
        .sum()
}

/// The least share of its redness the contact reflection keeps among faded rays.
const TIGHT: f32 = 0.9;

#[test]
fn gpu_a_satin_floor_keeps_a_standing_cubes_contact_reflection_tight() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let mut added = |wall| {
        let (on, off) = (
            shot(&mut r, &standing(true, wall), &down),
            shot(&mut r, &standing(false, wall), &down),
        );
        under_the_foot(&on, &off)
    };
    let (open, walled) = (added(false), added(true));
    let share = walled as f32 / open.max(1) as f32;
    eprintln!("[ssr] a cube standing on a roughness-0.35 floor: its reflection under its foot adds {open} red, {walled} with a far wall's faded rays around it ({share:.2})");
    assert!(open > 0, "the floor reflects the cube's foot");
    assert!(
        share >= TIGHT,
        "a contact reflection is not smeared by the long rays around it"
    );
}
