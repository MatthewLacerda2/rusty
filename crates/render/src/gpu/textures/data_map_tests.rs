use super::freshness_tests::{centre, emissive_scene, temp_png};
use crate::gpu::textures::TextureKind;
use rusty_procgen::bake::{bake_to_png, encode};
use rusty_procgen::{Image, Slot};

#[test]
fn gpu_a_16_bit_data_map_draws_like_its_8_bit_twin() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
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

#[test]
fn gpu_one_image_as_both_specular_maps_loads_once_as_colour_without_a_warning() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let packed = temp_png("packed_specular_2302");
    let img = Image::filled(4, [0.5, 0.25, 1.0, 0.75]);
    encode(&img, Slot::Data).save(&packed).unwrap();
    let mut scene = emissive_scene(&packed);
    let material = scene.materials.get_mut("glow").unwrap();
    material.specular_map = Some(packed.as_str().into());
    material.specular_color_map = Some(packed.as_str().into());

    centre(&mut renderer, &scene);
    let kinds = &renderer.texture_kinds;
    assert_eq!(kinds.kind_of(&packed), Some(TextureKind::Colour));
    assert!(!kinds.warned(&packed), "a packed pair is no clash");
    let fallback = renderer
        .texture_fallbacks
        .keys()
        .any(|m| m.as_str() == packed);
    assert!(
        !fallback,
        "one upload serves the level and the colour slots"
    );
}
