//! The grade's clamp before the tonemap, on lavapipe (#1440): knobs that push the
//! colour below zero or to infinity leave black black and highlights white,
//! whichever tonemapper runs.

use super::gpu_fixture::{chain, W};
use super::GradeParams;
use crate::components::{PostProcessingComponent, Tonemap, Wheel};
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing as ops;

/// A volume with `tonemap` and the `which` wheel set to `color` and `offset`.
fn wheel(tonemap: Tonemap, which: Wheel, color: [f32; 3], offset: f32) -> GradeParams {
    let mut vc = PostProcessingComponent {
        tonemap,
        ..default_post_processing()
    };
    ops::set_wheel(&mut vc, which, color, offset);
    GradeParams::from_volume(Some(&vc))
}

#[test]
fn gpu_a_negative_lift_keeps_black_black_under_either_tonemap() {
    let Some(mut c) = chain() else { return };
    let row = vec![[0.0f32; 3]; W as usize];
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard] {
        // Grey lifts below zero, and a cyan one whose red channel lands far below.
        for (color, offset) in [([1.0; 3], -0.1), ([1.0; 3], -0.3), ([0.0, 1.0, 1.0], -0.5)] {
            let out = c.run(&row, 0.0, &wheel(tonemap, Wheel::Lift, color, offset));
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
    // Black, then highlights well above display white.
    let mut row = vec![[10.0f32; 3]; W as usize];
    (row[0], row[1], row[2]) = ([0.0; 3], [4.0; 3], [40.0; 3]);
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard] {
        // Gamma offset −1: a per-channel power of 1000, which overflows any x > 1.
        let out = c.run(&row, 0.0, &wheel(tonemap, Wheel::Gamma, [1.0; 3], -1.0));
        assert_eq!(out[0], [0, 0, 0, 255], "{tonemap:?}: black stays black");
        for (i, px) in out.iter().enumerate().skip(1) {
            assert_eq!(*px, [255; 4], "{tonemap:?}: highlight {i} is white");
        }
    }
}
