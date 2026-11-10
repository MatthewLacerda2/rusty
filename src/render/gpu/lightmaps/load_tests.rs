//! What a direction page loads as (#1996): its RGBA8 KTX2 levels exactly as the
//! bake stored them, with or without BC compression; a preview's PNG its one
//! level as written; a page of another format, or a truncated one, refused naming it.

use glam::{Vec3, Vec4};

use super::load::{load, Kind};
use crate::asset::lightmap_page;
use crate::scene::lighting::lightmap::{colour_page, direction_page, encode_directions};

/// An `edge`² direction field turning across the page, with a lamp share.
fn directions(edge: u32) -> Vec<Vec4> {
    (0..edge * edge)
        .map(|i| {
            let x = (i % edge) as f32 / edge as f32 - 0.5;
            Vec3::new(x, 1.0, 0.2).normalize().extend(0.5 + x)
        })
        .collect()
}

/// Write `bytes` as `name` under the test's temp dir; its path.
fn file(name: &str, bytes: &[u8]) -> String {
    let dir = crate::test_temp::dir().join("rusty_1996");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn a_direction_ktx2_uploads_its_stored_levels_as_they_are() {
    let page = direction_page(&directions(16), 16);
    let path = file("dir.ktx2", &lightmap_page::write(&page));
    for bc in [true, false] {
        let loaded = load(std::slice::from_ref(&path), Kind::Direction, bc).unwrap();
        assert_eq!(loaded.format, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!((loaded.edge, &loaded.layers[0]), (16, &page.levels));
    }
}

#[test]
fn a_preview_direction_png_uploads_its_one_level_as_written() {
    let written = encode_directions(&directions(16));
    let png = image::RgbaImage::from_raw(16, 16, written.clone()).unwrap();
    let path = crate::test_temp::dir().join("rusty_1996_preview_dir.png");
    png.save(&path).unwrap();
    let loaded = load(&[path.to_string_lossy().into()], Kind::Direction, true).unwrap();
    assert_eq!(loaded.format, wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(
        (loaded.edge, &loaded.layers[0]),
        (16, &vec![written]),
        "no mips"
    );
}

#[test]
fn a_wrong_or_broken_direction_page_is_refused_naming_it() {
    let light = vec![Vec3::ONE; 16 * 16];
    let bc6h = colour_page(&light, 16, 1, &|| false).unwrap();
    let wrong = file("colour_as_dir.ktx2", &lightmap_page::write(&bc6h));
    let err = load(std::slice::from_ref(&wrong), Kind::Direction, true).unwrap_err();
    assert!(
        err.contains("colour_as_dir") && err.contains("Bc6h"),
        "{err}"
    );
    let whole = lightmap_page::write(&direction_page(&directions(16), 16));
    let cut = file("cut_dir.ktx2", &whole[..whole.len() - 3]);
    let err = load(&[cut], Kind::Direction, false).unwrap_err();
    assert!(err.contains("cut_dir"), "{err}");
}
