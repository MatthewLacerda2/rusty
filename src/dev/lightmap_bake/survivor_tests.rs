//! What #438's mutation run left unpinned (#820): a texture's share of the bake's
//! albedo, a page's content tag in its file name, and a cleanup that spares files
//! the bake did not write.

use super::tests::{quick, scene, scene_path};
use super::*;

/// The sRGB texels `pixels` (one row) written as a PNG in a fresh temp dir.
fn png(name: &str, pixels: &[[u8; 4]]) -> String {
    let dir = Path::new(&scene_path(name)).parent().unwrap().to_path_buf();
    let path = dir.join("albedo.png");
    let bytes = pixels.concat();
    image::RgbaImage::from_raw(pixels.len() as u32, 1, bytes)
        .unwrap()
        .save(&path)
        .unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn texture_average_is_the_linear_mean_of_its_texels() {
    // 188 is past the sRGB toe (≈ 0.5029 linear), 10 is inside it (10/255/12.92).
    let path = png("albedo", &[[188, 10, 0, 255], [188, 10, 255, 255]]);
    let average = texture_average(&path).expect("a readable PNG");
    assert!((average.x - 0.502_886).abs() < 1e-4, "{average}");
    assert!((average.y - 0.003_035_3).abs() < 1e-6, "{average}");
    assert!((average.z - 0.5).abs() < 1e-6, "{average}");
}

#[test]
fn an_unreadable_texture_has_no_average() {
    assert_eq!(texture_average("/nonexistent/rusty/albedo.png"), None);
}

#[test]
fn the_content_tag_is_fnv1a_folded_to_32_bits() {
    assert_eq!(fnv1a(b""), 0x4fd0_bfc1);
    assert_eq!(fnv1a(b"a"), 0x2962_30c0);
    assert_eq!(fnv1a(b"rusty"), 0xef78_44d0);
}

#[test]
fn a_page_whose_texels_change_changes_its_path() {
    let (mut scene, _, _) = scene();
    let path = scene_path("tag");
    let page = |scene: &mut Scene, seed| {
        let settings = BakeSettings { seed, ..quick() };
        bake_scene_lightmaps(scene, Some(&path), &settings).unwrap();
        scene.lightmaps.pages[0].clone()
    };
    let first = page(&mut scene, 1);
    let other = page(&mut scene, 2);
    assert_ne!(
        first, other,
        "new texels, new name: the by-path cache reloads"
    );
    assert_eq!(page(&mut scene, 1), first, "the same texels, the same name");
}

#[test]
fn a_rebake_spares_files_it_did_not_write() {
    let (mut scene, _, _) = scene();
    let path = scene_path("spare");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let old = scene.lightmaps.pages[0].clone();
    let dir = lightmap_dir(&path);
    let keep = [dir.join("notes.png"), dir.join("lightmap_readme.txt")];
    for file in &keep {
        std::fs::write(file, b"not a page").unwrap();
    }
    let reseeded = BakeSettings { seed: 2, ..quick() };
    bake_scene_lightmaps(&mut scene, Some(&path), &reseeded).unwrap();
    assert_ne!(scene.lightmaps.pages[0], old);
    assert!(!Path::new(&old).exists(), "the old page is removed");
    for file in &keep {
        assert!(file.exists(), "{} survives", file.display());
    }
}
