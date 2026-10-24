//! What a refresh may skip (#1256): an idle one sweeps nothing, a hand edit is
//! seen whatever it does to the count, and a file that cannot be read copies
//! nothing when it is retried.

use std::rc::Rc;

use glam::Vec2;

use super::tests::{fitted_image, temp_png, tick};
use super::TextureSizeReader;
use crate::core::fs::freshness::MISS_RETRY_FRAMES;
use crate::procgen::{bake::bake_to_png, Image, Slot};
use crate::ui::TextureSizes;

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

#[test]
fn a_texture_reference_measures_the_file_it_resolves_to() {
    let dir = crate::test_temp::dir();
    let source = dir.join("quad.fbx");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        crate::core::paths::native("tests/fixtures/models/embedded_textures.fbx"),
    );
    std::fs::copy(fixture, &source).expect("copy the fixture");
    let asset = crate::asset::import_file(&source).expect("imports");
    let red = asset.materials.iter().find(|m| m.name == "Red");
    let map = red.and_then(|m| m.base_color_map.clone()).expect("a map");
    assert!(
        map.source().is_some(),
        "an extracted texture is a reference"
    );
    let (w, h) = image::image_dimensions(map.file().expect("resolves")).unwrap();
    let (scene, id) = fitted_image(map.as_str());
    assert_eq!(
        tick(&scene, id, &mut fresh()),
        Vec2::new(w as f32, h as f32)
    );
}
