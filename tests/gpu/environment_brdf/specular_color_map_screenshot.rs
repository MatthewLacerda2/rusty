//! The specular colour map scales F0 alone: black reflects nothing head-on and keeps the
//! grazing rim, where a level map of 0 takes both. One whole-surface map per frame.

use glam::{Quat, Vec3};
use rusty::components::MaterialAsset;
use rusty::edit::authoring::Primitive;

use super::{black_plastic, reflectance, scene};

fn map(name: &str, rgba: [u8; 4]) -> String {
    let path = crate::temp::dir().join(format!("rusty_2302_{name}.png"));
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba(rgba));
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

/// `grazing_screenshot`'s slab, turned `degrees` from the camera.
fn seen(label: &str, degrees: f32, material: MaterialAsset) -> Option<f64> {
    let (mut scene, id) = scene(Primitive::Box, material);
    {
        let mut transform = scene.world.transform_mut(id).unwrap();
        transform.scale = Vec3::new(8.0, 1.0, 0.1);
        transform.rotation = Quat::from_rotation_y(degrees.to_radians());
    }
    reflectance(label, &scene)
}

fn plastic(color_map: Option<&str>, level_map: Option<&str>) -> MaterialAsset {
    MaterialAsset {
        specular_color_map: color_map.map(Into::into),
        specular_map: level_map.map(Into::into),
        ..black_plastic(0.05)
    }
}

#[test]
fn a_black_colour_map_takes_the_reflection_head_on_and_keeps_the_grazing_rim() {
    let (black, white) = (map("black", [0, 0, 0, 255]), map("white", [255; 4]));
    let level_zero = map("level_zero", [255, 255, 255, 0]);
    let cases = [
        ("none", plastic(None, None)),
        ("colour_black", plastic(Some(&black), None)),
        ("colour_white", plastic(Some(&white), None)),
        ("level_zero", plastic(None, Some(&level_zero))),
    ];
    let mut read = |degrees: f32| -> Option<Vec<f64>> {
        let at = |(name, m): &(&str, MaterialAsset)| {
            seen(&format!("2302_{name}_{degrees}"), degrees, m.clone())
        };
        cases.iter().map(at).collect()
    };
    let Some(head_on) = read(0.0) else {
        return;
    };
    let grazing = read(80.0).expect("the adapter rendered once");
    let [none, black, white, level] = head_on[..] else {
        unreachable!()
    };
    // lavapipe: none 0.0402, black 0, white 0.0402, level 0.
    assert!((0.02..0.07).contains(&none), "the default's F0: {none:.4}");
    assert!(
        black < 0.005 && level < 0.005,
        "black {black:.4}, level 0 {level:.4}"
    );
    assert!(
        (white - none).abs() < 0.002,
        "a white map is no map: {white:.4} vs {none:.4}"
    );
    let [none, black, white, level] = grazing[..] else {
        unreachable!()
    };
    // At 80° black's grazing term is 0.95 to white's 0.99, so about 0.92 of it. lavapipe:
    // none 0.4951, black 0.4532 (0.915), white 0.4951, level 0.
    assert!(white > 0.25, "the rim: {white:.4}");
    assert!(
        (white - none).abs() < 0.01,
        "white {white:.4} vs none {none:.4}"
    );
    assert!(
        black > 0.85 * white,
        "black keeps the rim: {black:.4} of {white:.4}"
    );
    assert!(level < 0.005, "a level map of 0 takes it: {level:.4}");
}
