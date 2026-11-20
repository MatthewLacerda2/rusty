//! What a refresh may skip (#1256): an idle one sweeps nothing, even while
//! something else writes other files (the write log is the process's, #2442), a
//! hand edit is seen whatever it does to the count, and a file that cannot be
//! read copies nothing when it is retried.

use std::rc::Rc;

use glam::Vec2;

use super::tests::{fitted_image, temp_png, tick};
use super::TextureSizeReader;
use crate::TextureSizes;
use rusty_core::fs::freshness::MISS_RETRY_FRAMES;
use rusty_procgen::{bake::bake_to_png, Image, Slot};

/// A fresh reader and an empty table.
fn fresh() -> (TextureSizeReader, Rc<TextureSizes>) {
    (TextureSizeReader::new(), Rc::default())
}

#[test]
fn an_idle_refresh_sweeps_nothing() {
    let path = temp_png("idle");
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    let (scene, id) = fitted_image(&path);
    let mut state = fresh();
    assert_eq!(tick(&scene, id, &mut state), Vec2::splat(4.0));
    assert_eq!(state.0.sweeps, 1);
    for _ in 0..3 {
        tick(&scene, id, &mut state);
    }
    assert_eq!(
        state.0.sweeps, 1,
        "no write, no retry, same Images: no sweep"
    );
}

#[test]
fn a_write_of_another_file_sweeps_nothing() {
    let path = temp_png("measured");
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    let (scene, id) = fitted_image(&path);
    let mut state = fresh();
    assert_eq!(tick(&scene, id, &mut state), Vec2::splat(4.0));
    let other = rusty_test_support::temp::dir().join("other.bin");
    for n in 0..3u8 {
        // What another test of the binary, or a background bake, does.
        let other = other.clone();
        std::thread::spawn(move || rusty_core::fs::written::write(&other, &[n]))
            .join()
            .expect("the writer ran")
            .expect("write");
        tick(&scene, id, &mut state);
    }
    assert_eq!(state.0.sweeps, 1, "the write log named no measured file");
}

#[test]
fn a_hand_edit_that_keeps_the_count_is_still_seen() {
    let path = temp_png("hand_edit");
    bake_to_png(&Image::new(4), Slot::BaseColor, &path).expect("bake");
    let (scene, id) = fitted_image(&path);
    let mut state = fresh();
    tick(&scene, id, &mut state);
    let table = Rc::make_mut(&mut state.1);
    table.remove(&path);
    table.set("elsewhere.png", Vec2::ONE);
    assert_eq!(tick(&scene, id, &mut state), Vec2::splat(4.0));
    assert_eq!(state.1.get("elsewhere.png"), None);
}

#[test]
fn a_file_that_cannot_be_read_copies_nothing_on_a_retry() {
    let path = temp_png("garbage");
    std::fs::write(&path, b"not a png").expect("write");
    let (scene, id) = fitted_image(&path);
    let mut state = fresh();
    assert_eq!(tick(&scene, id, &mut state), Vec2::ZERO);
    let held = Rc::clone(&state.1);
    for _ in 0..MISS_RETRY_FRAMES {
        tick(&scene, id, &mut state);
    }
    assert!(state.0.sweeps > 1, "the miss was retried");
    assert!(Rc::ptr_eq(&held, &state.1), "a failed read copies nothing");
}

/// `embedded_textures.fbx` imported from a temp copy: the copy's path and the
/// reference its `Red` material's map names.
fn imported_reference() -> (String, String) {
    let source = rusty_test_support::temp::dir().join("quad.fbx");
    let fixture =
        rusty_core::project::checkout().join("tests/fixtures/models/embedded_textures.fbx");
    std::fs::copy(fixture, &source).expect("copy the fixture");
    let asset = rusty_asset::import_file(&source).expect("imports");
    let red = asset.materials.iter().find(|m| m.name == "Red");
    let map = red.and_then(|m| m.base_color_map.clone()).expect("a map");
    assert!(
        map.source().is_some(),
        "an extracted texture is a reference"
    );
    (
        source.to_string_lossy().into_owned(),
        map.as_str().to_string(),
    )
}

#[test]
fn a_texture_reference_measures_the_file_it_resolves_to() {
    let (_, map) = imported_reference();
    let file = rusty_asset::convert::texture_ref::file(&map).expect("resolves");
    let (w, h) = image::image_dimensions(file).unwrap();
    let (scene, id) = fitted_image(&map);
    assert_eq!(
        tick(&scene, id, &mut fresh()),
        Vec2::new(w as f32, h as f32)
    );
}

#[test]
fn a_reconverted_source_remeasures_its_extracted_texture() {
    let (source, map) = imported_reference();
    let (scene, id) = fitted_image(&map);
    let mut state = fresh();
    let file = rusty_asset::convert::texture_ref::file(&map).expect("resolves");
    let (w, h) = image::image_dimensions(&file).unwrap();
    assert_eq!(tick(&scene, id, &mut state), Vec2::new(w as f32, h as f32));
    // A conversion writes the extracted file and notes the source, never the
    // reference: only the source names it in the write log.
    let (nw, nh) = (w + 3, h + 1);
    image::RgbaImage::new(nw, nh)
        .save(&file)
        .expect("re-extract");
    rusty_core::fs::written::note_written(&source);
    let size = tick(&scene, id, &mut state);
    assert_eq!(
        size,
        Vec2::new(nw as f32, nh as f32),
        "the source's re-conversion"
    );
}
