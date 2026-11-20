//! crates/render/src/scene/residency/notes_gpu_tests.rs — a settled frame notes no
//! map (#2458): the holders note what they bind on the render after a load, a Play
//! entry or a new upload, and on their own change, never on any other.

use glam::Vec3;

use super::holders_gpu_tests::{image, sprites};
use super::maps_gpu_tests::{png, textured};
use super::notes::notes;
use crate::Renderer;
use rusty_components::ribbon::LineComponent;
use rusty_scene::Scene;
use rusty_test_support::panorama::fixture as panorama;

/// A scene holding every kind of noted holder: a sky, a lightmap list, a UI image,
/// a sprite emitter and a textured line; the image's id.
fn every_holder(tag: &str) -> (Scene, u32) {
    let mut scene = textured(&[&png(&format!("{tag}_box"), [90; 3])]);
    scene.skybox_path = panorama(
        &format!("{tag}_sky.png"),
        [60, 100, 220],
        [160, 110, 60],
        40,
    );
    scene.lightmaps.pages = vec![format!("{tag}_page.png")];
    let ui = image(&mut scene, &png(&format!("{tag}_ui"), [200, 0, 0]));
    sprites(&mut scene, &png(&format!("{tag}_smoke"), [0; 3]));
    let id = scene.add_entity("Laser".to_string());
    let mut line = LineComponent {
        positions: vec![Vec3::new(-1.0, 0.0, -4.0), Vec3::new(1.0, 0.0, -4.0)],
        ..LineComponent::default()
    };
    line.style.texture = Some(png(&format!("{tag}_beam"), [0, 200, 0]));
    scene.world.set_line(id, Some(line));
    (scene, ui)
}

/// The note calls one frame of `scene` made.
fn noted(r: &mut Renderer, scene: &Scene) -> u64 {
    let before = notes();
    super::super::fixture::frame(r, scene);
    notes() - before
}

/// Draw `scene` until a frame uploads nothing new; the frames' note calls.
fn settle(r: &mut Renderer, scene: &Scene) -> Vec<u64> {
    (0..3).map(|_| noted(r, scene)).collect()
}

#[test]
fn gpu_a_settled_frame_notes_nothing() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (a, _) = every_holder("a");
    let drawn = settle(&mut r, &a);
    assert!(
        drawn[0] >= 5,
        "the first frame notes every holder: {drawn:?}"
    );
    assert_eq!(drawn[2], 0, "a settled frame notes nothing: {drawn:?}");
}

#[test]
fn gpu_a_load_and_a_play_entry_note_once() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (mut a, _) = every_holder("a");
    settle(&mut r, &a);
    // The same maps under a new scene: nothing uploads, so only the load is due.
    let (b, _) = every_holder("a");
    let loaded = settle(&mut r, &b);
    assert!(loaded[0] >= 5, "the load's frame notes them: {loaded:?}");
    assert_eq!(loaded[1..], [0, 0], "and then nothing: {loaded:?}");
    a.set_playing(true);
    let play = settle(&mut r, &a);
    assert!(play[0] >= 5, "the Play entry notes them: {play:?}");
    assert_eq!(play[1..], [0, 0], "and then nothing: {play:?}");
}

#[test]
fn gpu_a_changed_ui_image_is_noted_on_its_frame() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (mut a, ui) = every_holder("a");
    settle(&mut r, &a);
    let other = png("a_other", [0, 0, 200]);
    a.world.image_mut(ui).expect("image").texture = Some(other);
    let changed = settle(&mut r, &a);
    // Its frame notes the image whose group it rebuilt, and that alone; the upload
    // makes the next frame due, and the one after is settled.
    assert_eq!(
        changed[0], 1,
        "the rebuilt group notes the image: {changed:?}"
    );
    assert_eq!(changed[2], 0, "then nothing: {changed:?}");
}
