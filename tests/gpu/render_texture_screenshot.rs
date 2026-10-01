//! Render textures (#430), headless: a UI Image shows what a second camera sees,
//! and the texture's cost controls (unreferenced / `update_every`) skip its draws.

use rusty::components::RenderTarget;
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;

use super::render_texture_scene::{pip_scene, screen_camera, SIZE};

/// Red-dominant: the box, seen through the texture.
fn is_red(px: [u8; 3]) -> bool {
    px[0] > 60 && px[0] > px[1].saturating_add(40) && px[0] > px[2].saturating_add(40)
}

#[test]
fn an_image_shows_what_a_second_camera_sees() {
    let path = std::env::temp_dir().join("rusty_render_texture.png");
    let mut host = CaptureHost::new();
    let scene = pip_scene(Some(RenderTarget::new("pip", 64, 128)));
    let shot = capture_into(&mut host, &scene, &screen_camera(), &path, SIZE, SIZE);
    if !shot.expect("capture must not error") {
        eprintln!("[render texture] no GPU/software adapter — skipping");
        return;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let px = |x: u32, y: u32| img.get_pixel(x, y).0;
    // The Image's centre is the texture's centre: the box, straight ahead of it.
    assert!(is_red(px(32, 64)), "PiP centre: got {:?}", px(32, 64));
    // The screen's own camera looks away from the box.
    assert!(
        !is_red(px(96, 64)),
        "screen right half: got {:?}",
        px(96, 64)
    );
    let (counters, _) = host.last_frame.expect("a frame was drawn");
    assert_eq!(counters.render_texture_draws, 1);
}

#[test]
fn without_its_camera_the_image_shows_no_picture() {
    let path = std::env::temp_dir().join("rusty_render_texture_none.png");
    let mut host = CaptureHost::new();
    let shot = capture_into(
        &mut host,
        &pip_scene(None),
        &screen_camera(),
        &path,
        SIZE,
        SIZE,
    );
    if !shot.expect("capture must not error") {
        return;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    assert!(!is_red(img.get_pixel(32, 64).0), "no camera, no box");
    let (counters, _) = host.last_frame.expect("a frame was drawn");
    assert_eq!(counters.render_texture_draws, 0);
}

#[test]
fn update_every_skips_frames_between_draws() {
    let path = std::env::temp_dir().join("rusty_render_texture_every.png");
    let mut host = CaptureHost::new();
    let mut target = RenderTarget::new("pip", 64, 128);
    target.update_every = 2;
    let scene = pip_scene(Some(target));
    let mut draws = Vec::new();
    for _ in 0..3 {
        let shot = capture_into(&mut host, &scene, &screen_camera(), &path, SIZE, SIZE);
        if !shot.expect("capture must not error") {
            return;
        }
        draws.push(host.last_frame.expect("frame").0.render_texture_draws);
        // The held picture still shows on the frames that skip the draw.
        let img = image::open(&path).expect("png").to_rgb8();
        assert!(is_red(img.get_pixel(32, 64).0), "frame {}", draws.len());
    }
    assert_eq!(draws, [1, 0, 1]);
}
