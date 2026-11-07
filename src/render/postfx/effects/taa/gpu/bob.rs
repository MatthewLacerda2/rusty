//! #1846 on lavapipe: a viewmodel gun 0.6 m from its camera arriving over a wall
//! 0.35 m from the world's (a weapon raised, scaling in) is no reveal: a higher
//! camera's surface arriving never is one, though it is farther from its camera.
//! Bobbing off the wall is one, though the wall is nearer than the gun was: across
//! cameras the stack order decides, not the two distances.

use glam::Vec3;

use super::super::params::Rules;
use super::fixture::{diff, eye, frame, renderer, view};
use super::still::{run, NO_REVEAL};
use super::viewmodel::{green, wall_scene};
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::Renderer;

/// The gun's pose at frame `f`: x off its camera's axis and its size, metres.
type Pose = dyn Fn(u32) -> (f32, f32);

/// `wall_scene`'s gun settled at `pose(0)`, then posed by `pose` up to frame
/// `last`, with only `rules` kept: the settled frame and the last.
fn bob(r: &mut Renderer, pose: &Pose, last: u32, rules: Rules) -> (Vec<u8>, Vec<u8>) {
    let mut scene = wall_scene();
    // Blue noise, so the colour clamp's box is wide and the frame's weight shows.
    let wall = super::wall::noise([0.3, 0.3, 1.0], [2.0, 8.0]);
    scene.materials.insert("wall".to_string(), wall);
    let cam = scene.find_entity_by_name("GunCamera").unwrap();
    let gun = scene.world.children(cam)[0];
    let mut v = view(r);
    let place = |scene: &mut crate::scene::Scene, f: u32| {
        let (x, size) = pose(f);
        let mut t = scene.world.transform_mut(gun).unwrap();
        (t.position.x, t.scale) = (x, Vec3::splat(size));
    };
    place(&mut scene, 0);
    let settled = run(r, &mut v, (&scene, &|_| eye(0.0)), 8, rules);
    let mut end = settled.clone();
    for f in 1..=last {
        place(&mut scene, f);
        end = frame(r, &mut v, &scene, &eye(0.0));
    }
    (settled, end)
}

/// Every gun pixel of `frame`.
fn gun_pixels(frame: &[u8]) -> Vec<(u32, u32)> {
    let pixels = (0..H).flat_map(|y| (0..W).map(move |x| (x, y)));
    pixels.filter(|&p| green(frame, p)).collect()
}

#[test]
fn gpu_a_gun_arriving_over_a_nearer_wall_is_no_reveal() {
    let Some(mut r) = renderer() else { return };
    // Raised over the wall: from a speck to its size in four frames.
    let raise = |f: u32| (0.15, 0.001 + 0.2 * f as f32 / 4.0);
    let (_, on) = bob(&mut r, &raise, 4, Rules::default());
    let (_, off) = bob(&mut r, &raise, 4, NO_REVEAL);
    let gun = gun_pixels(&on);
    assert!(!gun.is_empty(), "the gun is over the wall");
    let worst = gun.iter().map(|&p| diff(&on, &off, p)).max().unwrap();
    eprintln!("[taa] the gun arriving over the wall differs from no reveal test by {worst}");
    assert_eq!(worst, 0, "the gun over the wall read as a reveal");
}

#[test]
fn gpu_a_gun_bobbing_off_a_nearer_wall_reveals_it() {
    let Some(mut r) = renderer() else { return };
    // 3 cm a frame, about two pixels, so a pixel it leaves finds no wall around it
    // in the history (a sub-pixel move always does: no reveal).
    let off_wall = |f: u32| (0.09 - 0.03 * f as f32, 0.2);
    let (start, on) = bob(&mut r, &off_wall, 3, Rules::default());
    let (_, off) = bob(&mut r, &off_wall, 3, NO_REVEAL);
    let left: Vec<_> = gun_pixels(&start)
        .into_iter()
        .filter(|&p| !green(&on, p))
        .collect();
    assert!(!left.is_empty(), "the gun left some wall pixels");
    let fired = left.iter().filter(|&&p| diff(&on, &off, p) > 0).count();
    eprintln!(
        "[taa] {fired} of {} wall pixels the gun left read as a reveal",
        left.len()
    );
    assert!(fired > 0, "the wall the gun left never read as a reveal");
}
