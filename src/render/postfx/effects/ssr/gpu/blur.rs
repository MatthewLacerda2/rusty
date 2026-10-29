//! Motion blur blurs the reflected scene (#1526): SSR runs on the base camera's
//! opaque image, before every post effect, as in HDRP.

use glam::Vec3;

use super::fixture::{draw, eye, paint, place, renderer, rgb, tuned, MIRROR};
use super::fixture::{H, W};
use crate::components::Tonemap;
use crate::render::{RenderView, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// [`eye`] turned `degrees` to the right.
fn turned(degrees: f32) -> Camera {
    Camera::new(eye().position, -90.0 + degrees, 0.0)
}

/// A grey cube on a mirror floor before a dark far wall, SSR on, its camera blurring
/// (`blur`) with the longest streaks it allows.
fn walled(blur: bool) -> Scene {
    let grey = ([0.3; 3], 0.0, 1.0);
    let (mut scene, cam) = tuned(true, MIRROR, grey, |v| v.tonemapping.mode = Tonemap::None);
    let wall = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let (at, size) = (Vec3::new(0.0, 4.0, -16.0), Vec3::new(30.0, 8.0, 0.1));
    place(&mut scene, wall, at, size);
    paint(&mut scene, wall, "wall", ([0.02; 3], 0.0, 1.0));
    let mut c = scene.world.camera_mut(cam).unwrap();
    (c.motion_blur_active, c.motion_blur_intensity) = (blur, 1.0);
    c.motion_blur_max_length = 0.2;
    drop(c);
    scene
}

/// The darkest channel of the darkest pixel within `reach` columns and one row of
/// `(x, y)`.
fn darkest_around(frame: &[u8], (x, y): (u32, u32), reach: u32) -> i32 {
    let columns = x.saturating_sub(reach)..(x + reach + 1).min(W);
    let rows = y.saturating_sub(1)..(y + 2).min(H);
    rows.flat_map(|ty| columns.clone().map(move |tx| (tx, ty)))
        .map(|p| *rgb(frame, p).iter().min().unwrap())
        .min()
        .unwrap()
}

#[test]
fn gpu_motion_blur_blurs_the_reflected_scene_never_below_what_it_blurs() {
    let Some(mut r) = renderer() else { return };
    let view =
        |r: &crate::render::Renderer| RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    let (blurring, still) = (walled(true), walled(false));
    let mut blurred_view = view(&r);
    draw(&mut r, &mut blurred_view, &blurring, &turned(0.0));
    let blurred = draw(&mut r, &mut blurred_view, &blurring, &turned(6.0));
    assert!(blurred_view.post_fx.holds_blur(), "the blur ran");
    let mut sharp_view = view(&r);
    let sharp = draw(&mut r, &mut sharp_view, &still, &turned(6.0));
    assert!(!sharp_view.post_fx.holds_blur());

    // The blur averages taps of the reflected image along each pixel's streak (at
    // most 9.6 px each way here), so no pixel may come out darker than the darkest
    // tap around it. SSR composited after the blur instead subtracted the sharp
    // floor's reflection from the cube smeared over it, streaking its edges black.
    let mut worst = 0;
    for y in 0..H {
        for x in 0..W {
            let here = *rgb(&blurred, (x, y)).iter().min().unwrap();
            worst = worst.max(darkest_around(&sharp, (x, y), 12) - here);
        }
    }
    let moved = (0..H * W).filter(|i| rgb(&blurred, (i % W, i / W)) != rgb(&sharp, (i % W, i / W)));
    eprintln!(
        "[ssr] motion blur: {} pixels moved, at worst {worst} below the darkest tap",
        moved.count()
    );
    assert!(worst <= 3, "SSR darkened the blurred image: {worst}");
}
