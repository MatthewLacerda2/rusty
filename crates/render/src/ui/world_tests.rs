use glam::Vec2;

use super::*;
use crate::ui::clip::Scissor;

#[test]
fn a_scissor_maps_to_the_canvas_ndc_its_vertices_use() {
    let frame = Vec2::new(400.0, 200.0);
    let whole = Scissor {
        x: 0,
        y: 0,
        w: 400,
        h: 200,
    };
    assert_eq!(whole.ndc(frame), [-1.0, -1.0, 1.0, 1.0]);
    let quarter = Scissor {
        x: 200,
        y: 0,
        w: 200,
        h: 100,
    };
    assert_eq!(quarter.ndc(frame), [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn a_slot_fits_one_alignment_step() {
    assert!(std::mem::size_of::<WorldUiUniform>() <= 256);
}
