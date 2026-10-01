use glam::Vec2;

use super::*;
use crate::render::ui::mesh::Scissor;

#[test]
fn a_scissor_maps_to_the_canvas_ndc_its_vertices_use() {
    // A 400×200 frame; the scissor's top-left origin flips to NDC's bottom-up.
    let frame = Vec2::new(400.0, 200.0);
    let whole = Scissor {
        x: 0,
        y: 0,
        w: 400,
        h: 200,
    };
    assert_eq!(whole.ndc(frame), [-1.0, -1.0, 1.0, 1.0]);
    // The top-right quarter: x 200..400, y (top-down) 0..100.
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
    // wgpu's default `min_uniform_buffer_offset_alignment` is 256: one per batch.
    assert!(std::mem::size_of::<WorldUiUniform>() <= 256);
}
