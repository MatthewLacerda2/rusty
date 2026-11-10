//! A short march keeps its reach (#2004): the screen-step cap is `1/steps` of the
//! screen below 64 steps, so the `Low` and `Medium` levels' 16 and 32 steps still
//! cross the screen. Held to 1/64 at any count, 16 steps crossed a quarter of it,
//! and a mirror floor's reflection of a tall cube stopped partway down the frame.

use glam::Vec3;

use super::fixture::{place, tuned, RED};
use super::silhouette::{at, shot, H, W};
use crate::core::settings::quality::EffectQuality;
use crate::render::Renderer;
use crate::scene::Camera;

/// How many pixels of the frame's lower half are red with SSR on at a march of
/// `steps` × `len` on a mirror floor, a 4 m tall red cube 7 m out.
fn reflected(r: &mut Renderer, steps: u32, len: f32) -> usize {
    let (mut s, _) = tuned(true, ([0.9; 3], 1.0, 0.0), RED, |v| {
        v.ssr.quality = EffectQuality::Custom;
        (v.ssr.steps, v.ssr.step_length) = (steps, len);
    });
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(
        &mut s,
        cube,
        Vec3::new(0.0, 2.0, -7.0),
        Vec3::new(1.5, 4.0, 1.5),
    );
    let frame = shot(r, &s, &Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, -8.0));
    (H / 2..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let c = at(&frame, x, y);
            c[0] > c[2] + 40
        })
        .count()
}

#[test]
fn gpu_sixteen_and_thirty_two_steps_reflect_as_far_as_sixty_four() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    // The seeded `Medium` preset's three levels.
    let full = reflected(&mut r, 64, 0.25);
    for (steps, len) in [(16, 0.5), (32, 0.375)] {
        let n = reflected(&mut r, steps, len);
        eprintln!("[ssr] reach: {steps} steps {n} red px, 64 steps {full}");
        assert!(n * 100 >= full * 98, "{steps} steps reflect as far as 64");
    }
}
