use super::{chain, W};
use rusty_components::{PostProcessingComponent, Tonemap, Wheel};
use rusty_scene::create::default_post_processing;

fn wheel(tonemap: Tonemap, which: Wheel, color: [f32; 3], offset: f32) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.tonemapping.mode = tonemap;
    let wheel = vc.lift_gamma_gain.wheel_mut(which);
    (wheel.color, wheel.offset) = (color, offset);
    vc
}

#[test]
fn gpu_a_negative_lift_keeps_black_black_under_every_tonemap() {
    let Some(mut c) = chain() else { return };
    let row = vec![[0.0f32; 3]; W as usize];
    for tonemap in Tonemap::ALL {
        for (color, offset) in [([1.0; 3], -0.1), ([1.0; 3], -0.3), ([0.0, 1.0, 1.0], -0.5)] {
            let out = c.run(&row, 0.0, Some(&wheel(tonemap, Wheel::Lift, color, offset)));
            assert_eq!(
                out[0],
                [0, 0, 0, 255],
                "{tonemap:?}, lift {color:?} {offset}: black stays black"
            );
        }
    }
}

#[test]
fn gpu_a_gamma_wheel_at_its_floor_leaves_highlights_white_not_nan() {
    let Some(mut c) = chain() else { return };
    let mut row = vec![[10.0f32; 3]; W as usize];
    (row[0], row[1], row[2]) = ([0.0; 3], [4.0; 3], [40.0; 3]);
    for tonemap in Tonemap::ALL {
        let out = c.run(
            &row,
            0.0,
            Some(&wheel(tonemap, Wheel::Gamma, [1.0; 3], -1.0)),
        );
        assert_eq!(out[0], [0, 0, 0, 255], "{tonemap:?}: black stays black");
        for (i, px) in out.iter().enumerate().skip(1) {
            assert_eq!(*px, [255; 4], "{tonemap:?}: highlight {i} is white");
        }
    }
}
