//! A texture a conversion extracted is compressed like any image (#2365): an
//! import makes its copy, by the slot its model samples it in, beside the model's
//! cache, re-made only when its pixels change, and kept by the orphan sweep while
//! the model names it.

use std::path::{Path, PathBuf};

use super::orphans::orphans;
use super::*;
use crate::convert::texture_ref::extracted;
use rusty_test_support::models::{embedded_fbx, png, Embedded};

/// `dir/m.fbx` embedding an 8² albedo of `seed` and, with `normal`, an 8² normal
/// map, converted: its cached glTF.
fn model(dir: &Path, seed: u8, normal: bool) -> PathBuf {
    let (albedo, bumps) = (png(8, seed), png(8, 77));
    let mut maps = vec![Embedded {
        name: "Albedo",
        property: "DiffuseColor",
        png: &albedo,
    }];
    if normal {
        maps.push(Embedded {
            name: "Bumps",
            property: "NormalMap",
            png: &bumps,
        });
    }
    let fbx = dir.join("m.fbx");
    std::fs::write(&fbx, embedded_fbx(&maps)).unwrap();
    crate::convert::cached_gltf(&fbx).expect("the model converts")
}

/// The extracted image of `glb` named `name`.
fn image(glb: &Path, name: &str) -> PathBuf {
    let found = extracted::images(glb).into_iter();
    found
        .into_iter()
        .find(|i| i.file_name().is_some_and(|n| n == name))
        .unwrap_or_else(|| panic!("{name} extracted"))
}

/// The block format of the current copy of `image`, as an upload reads it.
fn format(image: &Path, kind: TextureKind) -> Option<BlockFormat> {
    let settings = crate::sidecar::texture(image);
    match load(image, &settings, kind, true)?.texels {
        Texels::Blocks(blocks) => Some(blocks.format),
        Texels::Image(_) => None,
    }
}

#[test]
fn an_import_compresses_a_models_extracted_textures_by_their_slot() {
    let dir = rusty_test_support::temp::dir();
    let glb = model(&dir, 1, true);
    let (albedo, bumps) = (image(&glb, "Albedo.png"), image(&glb, "Bumps.png"));
    assert_eq!(images(&dir), [albedo.clone(), bumps.clone()]);
    assert_eq!(format(&albedo, TextureKind::Colour), None, "not made yet");
    assert_eq!(refresh_all(&dir).encoded, 2);
    assert_eq!(format(&albedo, TextureKind::Colour), Some(BlockFormat::Bc7));
    assert_eq!(
        format(&bumps, TextureKind::Normal),
        Some(BlockFormat::Bc5),
        "a normal map is not encoded as colour"
    );
    let copy = cache_file(&albedo).unwrap();
    assert_eq!(copy, extracted_cache(&glb, &albedo));
    assert!(
        copy.is_file(),
        "beside the model's cache: {}",
        copy.display()
    );
}

#[test]
fn a_reimport_re_encodes_only_the_image_that_changed() {
    let dir = rusty_test_support::temp::dir();
    model(&dir, 1, true);
    assert_eq!(refresh_all(&dir).encoded, 2);
    let glb = model(&dir, 2, true);
    assert_eq!(
        refresh_all(&dir).encoded,
        1,
        "the normal map's copy is current in its new folder"
    );
    let albedo = image(&glb, "Albedo.png");
    assert_eq!(format(&albedo, TextureKind::Colour), Some(BlockFormat::Bc7));
    assert_eq!(refresh_all(&dir), Refreshed::default());
}

#[test]
fn the_orphan_sweep_keeps_an_extracted_textures_copy_until_its_model_drops_it() {
    let dir = rusty_test_support::temp::dir();
    let glb = model(&dir, 1, true);
    refresh_all(&dir);
    let bumps = extracted_cache(&glb, &image(&glb, "Bumps.png"));
    let folder = bumps.parent().unwrap().to_path_buf();
    assert!(orphans(&folder, &dir).is_empty(), "both images are named");
    let glb = model(&dir, 1, false);
    assert_eq!(orphans(&folder, &dir), [bumps], "re-converted without it");
    std::fs::remove_file(&glb).unwrap();
    assert_eq!(orphans(&folder, &dir).len(), 2, "the model's cache is gone");
}

#[test]
fn imports_counts_an_extracted_texture_of_a_model_under_the_root() {
    let dir = rusty_test_support::temp::dir();
    let cache = dir.join(crate::convert::IMPORT_CACHE_DIR);
    let folder = cache.join("models/m.fbx.glb.0123456789abcdef.textures");
    let tex = folder.join("skin.png");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(&tex, png(8, 0)).unwrap();
    assert!(
        imports(&dir.join("models"), &tex),
        "its model is under the root"
    );
    assert!(!imports(&dir.join("other"), &tex), "its model is not");
}

#[test]
fn an_editor_side_conversion_queues_its_extracted_textures() {
    model(&rusty_test_support::temp::dir(), 1, true);
    assert_eq!(super::background::take_queued(), Vec::<PathBuf>::new());
    import_in_background();
    let glb = model(&rusty_test_support::temp::dir(), 2, true);
    let queued = super::background::take_queued();
    assert_eq!(
        queued,
        extracted::images(&glb),
        "a model with no cache, converted"
    );
    assert_eq!(queued.len(), 2);
}
