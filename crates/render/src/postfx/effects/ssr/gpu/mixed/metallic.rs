//! A `metallic = 1` cube whose metallic map is 0 on one half is reflected lit there.

use super::super::fixture::{rgb, H, MIRROR, W};
use super::{room, shot};
use crate::Renderer;
use rusty_scene::Scene;

/// Metallic in blue (glTF's): 0 over u < 0.5, else 1. Red is its opposite, to catch a swap.
fn half_metal_map() -> String {
    let img = image::RgbaImage::from_fn(8, 8, |x, _| {
        let metal = if x < 4 { 0 } else { 255 };
        image::Rgba([255 - metal, 128, metal, 255])
    });
    let path = rusty_test_support::temp::dir().join("rusty_ssr_rays_half_metal.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn green_map() -> String {
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 220, 20, 255]));
    let path = rusty_test_support::temp::dir().join("rusty_ssr_rays_metal_green.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

fn metal_room(metallic_map: Option<&str>, base_color_map: Option<&str>) -> Scene {
    let mut s = room(MIRROR, true, |_| {});
    let cube = s.materials.get_mut("cube").unwrap();
    (cube.emissive, cube.base_color, cube.metallic) = ([0.0; 3], [1.0; 3], 1.0);
    cube.metallic_map = metallic_map.map(|m| m.to_string().into());
    cube.base_color_map = base_color_map.map(|m| m.to_string().into());
    s
}

/// The middle third's mean colour: the map's 0 half (left), its 1 half, clear of the seam.
fn halves(frame: &[u8]) -> [[i32; 3]; 2] {
    let half = |xs: std::ops::Range<u32>| {
        let all: Vec<_> = (H / 3..2 * H / 3)
            .flat_map(|y| xs.clone().map(move |x| (x, y)))
            .collect();
        let sum = all.iter().fold([0; 3], |a, &p| {
            let c = rgb(frame, p);
            [a[0] + c[0], a[1] + c[1], a[2] + c[2]]
        });
        sum.map(|c| c / all.len() as i32)
    };
    [half(W / 3..W / 2 - SEAM), half(W / 2 + SEAM..2 * W / 3)]
}

const SEAM: u32 = 4;

fn lit(c: [i32; 3]) -> bool {
    c.iter().any(|&v| v > 20)
}

/// The 1 half reads at most 6 on Metal.
fn dark(c: [i32; 3]) -> bool {
    c.iter().all(|&v| v < 12)
}

fn split(r: &mut Renderer, scene: &Scene, what: &str) -> [[i32; 3]; 2] {
    let [zero, one] = halves(&shot(r, scene));
    eprintln!("[ssr] mixed metallic map, {what}: 0 half {zero:?}, 1 half {one:?}");
    [zero, one]
}

#[test]
fn gpu_mixed_shades_a_hit_by_its_metallic_map_with_or_without_a_base_colour_map() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let metal = half_metal_map();
    let [zero, one] = split(&mut r, &metal_room(None, None), "none");
    assert!(dark(zero) && dark(one), "a full metal: {zero:?} {one:?}");
    let [zero, one] = split(&mut r, &metal_room(Some(&metal), None), "alone");
    assert!(lit(zero), "the map's 0 half is reflected lit: {zero:?}");
    assert!(dark(one), "its 1 half, a metal, black: {one:?}");
    let green = green_map();
    let both = metal_room(Some(&metal), Some(&green));
    let [zero, one] = split(&mut r, &both, "with a base colour map");
    let [red, g, blue] = zero;
    assert!(
        g > red + 20 && g > blue + 20,
        "the 0 half is green: {zero:?}"
    );
    assert!(dark(one), "the 1 half stays black: {one:?}");
}

#[test]
fn gpu_mixed_follows_a_metallic_map_added_and_removed_by_an_edit() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let mut scene = metal_room(None, None);
    let [zero, _] = split(&mut r, &scene, "before the edit");
    assert!(dark(zero), "a full metal: {zero:?}");
    let metal = half_metal_map();
    scene.materials.get_mut("cube").unwrap().metallic_map = Some(metal.into());
    let [zero, _] = split(&mut r, &scene, "added");
    assert!(lit(zero), "the added map's 0 half is lit: {zero:?}");
    scene.materials.get_mut("cube").unwrap().metallic_map = None;
    let [zero, _] = split(&mut r, &scene, "removed");
    assert!(dark(zero), "removed, a full metal again: {zero:?}");
}
