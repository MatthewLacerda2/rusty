//! A `data` bake is a 16-bit PNG (#1491); the texture loader still reads it as a
//! material map, drawing exactly what its 8-bit twin draws.

use super::freshness_tests::{centre, emissive_scene, temp_png};
use crate::procgen::bake::{bake_to_png, encode};
use crate::procgen::{Image, Slot};

#[test]
fn gpu_a_16_bit_data_map_draws_like_its_8_bit_twin() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let img = Image::filled(4, [0.5, 0.25, 1.0, 1.0]);
    let wide = temp_png("data_16");
    bake_to_png(&img, Slot::Data, &wide).unwrap();
    assert_eq!(
        image::open(&wide).unwrap().color(),
        image::ColorType::Rgba16
    );
    let narrow = temp_png("data_8");
    encode(&img, Slot::Data).save(&narrow).unwrap();

    let drawn_wide = centre(&mut renderer, &emissive_scene(&wide));
    let drawn_narrow = centre(&mut renderer, &emissive_scene(&narrow));
    assert_eq!(drawn_wide, drawn_narrow);
    let tinted = drawn_wide[0] < 200 && drawn_wide[2] > 200;
    assert!(tinted, "the map was drawn, not the default: {drawn_wide:?}");
}
