//! A ray hit reads its material's base colour map from the packed array.

use super::fixture::{rgb, H, MIRROR, W};
use super::mixed::{room, shot};

fn green_map() -> String {
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 220, 20, 255]));
    let path = rusty_test_support::temp::dir().join("rusty_ssr_rays_green.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// The middle third's mean colour.
fn mean(frame: &[u8]) -> [i32; 3] {
    let (xs, ys) = (W / 3..2 * W / 3, H / 3..2 * H / 3);
    let all: Vec<_> = ys.flat_map(|y| xs.clone().map(move |x| (x, y))).collect();
    let sum = all.iter().fold([0; 3], |a, &p| {
        let c = rgb(frame, p);
        [a[0] + c[0], a[1] + c[1], a[2] + c[2]]
    });
    sum.map(|c| c / all.len() as i32)
}

#[test]
fn gpu_mixed_shades_a_hit_with_its_base_colour_map() {
    let Some(mut r) = super::fixture::renderer() else {
        return;
    };
    let lit = |map: Option<String>| {
        let mut s = room(MIRROR, true, |_| {});
        let cube = s.materials.get_mut("cube").unwrap();
        (cube.emissive, cube.base_color) = ([0.0; 3], [1.0; 3]);
        cube.base_color_map = map.map(Into::into);
        s
    };
    let plain = mean(&shot(&mut r, &lit(None)));
    let mapped = mean(&shot(&mut r, &lit(Some(green_map()))));
    eprintln!("[ssr] mixed base colour: plain {plain:?}, mapped {mapped:?}");
    assert!(plain[1] > 20, "the plain cube is lit: {plain:?}");
    assert!(
        mapped[1] > mapped[0] + 20 && mapped[1] > mapped[2] + 20,
        "the green map: {mapped:?}"
    );
}
