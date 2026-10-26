//! A render texture whose camera runs only the display transform
//! (`Camera.SetTargetPostFx(id, false)`) gets none of its volume's lens effects
//! (#1439): the vignette, chromatic aberration and film grain are post-FX like the
//! grade, so the tonemap-only stack drops them too.

use crate::temp::dir;
use rusty::components::{PostProcessingComponent, RenderTarget};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::authoring::default_post_processing;

use super::super::render_texture_scene::{pip_scene, screen_camera, SIZE};

/// Every lens effect on at full strength, the vignette a bright green.
fn lens_on() -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.vignette.active = true;
    vc.vignette.intensity = 1.0;
    vc.vignette.color = [0.0, 1.0, 0.0];
    vc.chromatic_aberration.active = true;
    vc.chromatic_aberration.intensity = 1.0;
    vc.film_grain.active = true;
    vc.film_grain.intensity = 1.0;
    vc
}

/// The PiP scene, its texture camera running full post-FX or not, with `vc`.
fn shot(full: bool, vc: PostProcessingComponent, name: &str) -> Option<Vec<u8>> {
    let mut target = RenderTarget::new("pip", 64, 128);
    target.post_fx = full;
    let mut scene = pip_scene(Some(target));
    let cam = scene.find_entity_by_name("PipCamera").expect("pip camera");
    scene.world.set_post_processing(cam, Some(vc));
    let path = dir().join(format!("rusty_lens_tonemap_only_{name}.png"));
    let host = &mut CaptureHost::new();
    let ok = capture_into(host, &scene, &screen_camera(), &path, SIZE, SIZE);
    ok.expect("capture").then(|| std::fs::read(path).unwrap())
}

#[test]
fn a_tonemap_only_render_texture_draws_no_lens_effects() {
    let Some(plain) = shot(false, default_post_processing(), "plain") else {
        eprintln!("[lens tonemap only] no GPU/software adapter — skipping");
        return;
    };
    let lens = shot(false, lens_on(), "lens").expect("adapter");
    assert_eq!(plain, lens, "tonemap only: no vignette, fringes or grain");
    let full_plain = shot(true, default_post_processing(), "full_plain").expect("adapter");
    let full_lens = shot(true, lens_on(), "full_lens").expect("adapter");
    assert_ne!(
        full_plain, full_lens,
        "the lens effects reach a full texture"
    );
}
