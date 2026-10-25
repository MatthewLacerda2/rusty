//! A UI Image samples with its texture's import settings (#1397): a `Point`
//! checker magnified 24× stays two flat colours, the default (Trilinear) one blends
//! at its cell edges, and both clamp, so an image's border never bleeds the
//! opposite edge in. Asserted at known pixels (`px`: bottom-left origin).

use glam::Vec4;
use rusty::asset::sidecar::{set_texture, FilterMode, TextureSettings};
use rusty::scene::Scene;

use super::super::render_texture_scene::screen_camera;
use super::mask_scene::{element, overlay, px, shoot};

/// A `cells`² black/white checker sprite written to disk with `filter` in its
/// sidecar (the rest left at the defaults: Repeat, mips), returned as its path.
fn checker(cells: u32, filter: FilterMode) -> String {
    let name = format!("rusty_ui_checker_{cells}_{filter:?}.png");
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(cells, cells, |x, y| match (x + y) % 2 {
        0 => image::Rgba([255, 255, 255, 255]),
        _ => image::Rgba([0, 0, 0, 255]),
    });
    img.save(&path).expect("write the checker sprite");
    let settings = TextureSettings {
        filter_mode: filter,
        ..Default::default()
    };
    set_texture(&path, settings).expect("write the sidecar");
    path.to_string_lossy().into_owned()
}

/// A 96-unit square at (16, 16) showing `texture`: each of a 4² checker's cells is
/// 24 pixels wide.
fn shot(texture: String, name: &str) -> Option<image::RgbImage> {
    let mut scene = Scene::new();
    let root = overlay(&mut scene);
    let id = element(&mut scene, root, [16.0, 16.0, 96.0, 96.0], Vec4::ONE);
    scene.world.image_mut(id).expect("image").texture = Some(texture.into());
    shoot(&scene, &screen_camera(), name).map(|(img, _)| img)
}

/// The pixels of row `y` across the image's 96 columns that are neither black nor
/// white: the blend a linear filter draws at each cell edge.
fn greys(img: &image::RgbImage, y: u32) -> usize {
    let row = (16..112).map(|x| px(img, x, y)[0]);
    row.filter(|&v| (24..232).contains(&v)).count()
}

#[test]
fn a_point_filtered_ui_image_draws_crisp_texels() {
    let Some(img) = shot(checker(4, FilterMode::Point), "rusty_ui_point.png") else {
        return;
    };
    assert_eq!(greys(&img, 52), 0, "Point: no blended column");
    // The bottom-left cell is (0, 3) in the image: black; its neighbour white.
    assert!(px(&img, 17, 17)[0] < 24, "{:?}", px(&img, 17, 17));
    assert!(px(&img, 41, 17)[0] > 232, "{:?}", px(&img, 41, 17));
}

#[test]
fn a_default_ui_image_blends_and_clamps_its_edges() {
    let Some(img) = shot(checker(4, FilterMode::Trilinear), "rusty_ui_trilinear.png") else {
        return;
    };
    assert!(
        greys(&img, 52) >= 6,
        "Trilinear blends: {}",
        greys(&img, 52)
    );
    // A repeating sampler would blend the white right-hand column into the
    // black left edge; clamped, the edge keeps its own texel.
    let edge = px(&img, 16, 28);
    assert!(edge[0] < 64, "the left edge clamps: {edge:?}");
}
