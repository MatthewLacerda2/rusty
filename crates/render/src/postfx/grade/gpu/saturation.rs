use super::{chain, W};
use rusty_components::{PostProcessingComponent, Tonemap};
use rusty_scene::create::default_post_processing;

fn at(saturation: f32) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.tonemapping.mode = Tonemap::Aces;
    vc.color_adjustments.saturation = saturation;
    vc
}

#[test]
fn gpu_saturation_runs_before_the_tonemap_and_holds_channels_at_black() {
    let Some(mut c) = chain() else { return };
    let row = vec![[2.0f32, 0.1, 0.1]; W as usize];
    let neutral = c.run(&row, 0.0, Some(&at(1.0)))[0];
    let vivid = c.run(&row, 0.0, Some(&at(1.5)))[0];
    let grey = c.run(&row, 0.0, Some(&at(0.0)))[0];
    eprintln!("[saturation] 1 {neutral:?}, 1.5 {vivid:?}, 0 {grey:?}");

    assert_eq!((vivid[1], vivid[2]), (0, 0), "1.5 empties green and blue");
    assert!(vivid[0] >= neutral[0], "1.5 keeps the red");
    assert!(neutral[1] > 0, "the neutral grade keeps some green");
    assert_eq!((grey[0], grey[2]), (grey[1], grey[1]), "0 is grey");
}
