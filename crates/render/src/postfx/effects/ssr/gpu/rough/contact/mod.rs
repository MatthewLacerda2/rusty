//! A contact reflection among long faded rays (#2158): a cube, or a thin post,
//! standing on a satin metal floor, a far wall behind it. Most of the floor's rays
//! run to the wall and fade out (their blur too wide to keep), but they still tell
//! the resolve how wide a blur is around them. The rays that meet a cube's foot are
//! enough of the mean around it to keep its reflection there tight; a thin post's
//! are not, and its reflection fades with the long rays' (the radius field is a
//! plain mean, smooth among hits: `docs/api/Graphics.md`).

use glam::Vec3;

use super::super::fixture::{paint, place};
use super::super::silhouette::{base, BRONZE};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::MaterialAsset;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

mod dark;

/// What stands on the floor: the floor's roughness, a thin post in the cube's
/// place, and a dark cube before a glowing wall in place of a red one before a
/// lit wall.
#[derive(Clone, Copy)]
struct Case {
    roughness: f32,
    thin: bool,
    dark: bool,
}

/// [`base`]'s cube standing on a floor of the case's roughness, or a post 8 cm
/// thick and 1.2 m tall in its place when `thin`, with or without a `wall` 16 m
/// out; when `dark`, the cube a twelfth as bright, the wall glowing blue and
/// the sky black, so the probe's reflection holds no red.
fn standing(on: bool, wall: bool, case: Case) -> Scene {
    let Case {
        roughness,
        thin,
        dark,
    } = case;
    let mut s = base(on, (BRONZE, 1.0, roughness));
    if dark {
        crate::test_gpu::black_sky(&mut s);
        let cube = s.materials.get_mut("cube").unwrap();
        cube.base_color = cube.base_color.map(|c| c / 12.0);
    }
    if thin {
        let post = s.find_entity_by_name("Cube").unwrap();
        let (at, size) = (Vec3::new(0.0, 0.6, -6.0), Vec3::new(0.08, 1.2, 0.08));
        place(&mut s, post, at, size);
    }
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
    if dark {
        let glow = MaterialAsset {
            emissive: [0.0, 1.0, 12.0],
            ..s.materials["wall"].clone()
        };
        s.materials.insert("wall".into(), glow);
    }
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
/// the rows down to a fortieth of the frame below it, the foot found in `bare`
/// (the red cube with SSR off and no wall).
/// When `dark`, the red channel alone: the wall's blue glow blurs over the foot
/// as its rays do and would read as lost redness.
fn under_the_foot(on: &[u8], off: &[u8], (bare, dark): (&[u8], bool)) -> i32 {
    let added = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        let only = i32::from(on[i]) - i32::from(off[i]);
        if dark {
            only
        } else {
            red(on, x, y) - red(off, x, y)
        }
    };
    let cube = |y: u32| (0..W).filter(move |&x| red(bare, x, y) > 60);
    let foot = (0..H).filter(|&y| cube(y).next().is_some()).max();
    let foot = foot.expect("the cube is in view");
    let (left, right) = (cube(foot).min().unwrap(), cube(foot).max().unwrap());
    let rows = foot + 2..foot + 2 + H / 40;
    rows.flat_map(|y| (left..=right).map(move |x| (x, y)))
        .map(|(x, y)| added(x, y))
        .sum()
}

/// The share of the redness the reflection under the foot of `case` keeps with
/// the far wall's faded rays around it, against none; and the redness with none.
fn kept(r: &mut Renderer, case: Case) -> (f32, i32) {
    let down = Camera::new(Vec3::new(0.0, 1.0, 0.0), -90.0, -12.0);
    let bare = shot(
        r,
        &standing(
            false,
            false,
            Case {
                dark: false,
                ..case
            },
        ),
        &down,
    );
    let mut added = |wall| {
        let (on, off) = (
            shot(r, &standing(true, wall, case), &down),
            shot(r, &standing(false, wall, case), &down),
        );
        under_the_foot(&on, &off, (&bare, case.dark))
    };
    let (open, walled) = (added(false), added(true));
    (walled as f32 / open.max(1) as f32, open)
}

/// The least share of its redness a cube's contact reflection keeps among faded rays.
const TIGHT: f32 = 0.9;

#[test]
fn gpu_a_satin_floor_keeps_a_standing_cubes_contact_reflection_tight() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (share, open) = kept(
        &mut r,
        Case {
            roughness: 0.35,
            thin: false,
            dark: false,
        },
    );
    eprintln!("[ssr] a cube standing on a roughness-0.35 floor: its reflection under its foot adds {open} red, keeping {share:.2} of it with a far wall's faded rays around it");
    assert!(open > 0, "the floor reflects the cube's foot");
    assert!(
        share >= TIGHT,
        "a contact reflection is not smeared by the long rays around it"
    );
}

/// What a thin post's contact reflection keeps on a rougher floor: on its own, a
/// reflection under its foot; among a far wall's long rays, their blur, so it fades
/// to the probe's reflection with theirs (near none of it here), never a patch
/// brighter or darker than that reflection was.
#[test]
fn gpu_a_thin_posts_contact_reflection_fades_with_the_long_rays_around_it() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (share, open) = kept(
        &mut r,
        Case {
            roughness: 0.5,
            thin: true,
            dark: false,
        },
    );
    eprintln!("[ssr] a thin post on a roughness-0.5 floor: its reflection under its foot adds {open} red, keeping {share:.2} of it with a far wall's faded rays around it");
    assert!(open > 0, "the floor reflects the post's foot");
    assert!(
        share.abs() <= 1.0,
        "among long rays a thin contact fades, adding or taking no more than it had"
    );
}
