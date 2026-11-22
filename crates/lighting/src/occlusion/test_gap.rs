//! Test hooks: the gap scene other crates' occlusion tests bake.

use super::test_level::{block, DOOR, HEIGHT, WALL};
use rusty_scene::Scene;

pub fn gap(length: f32, x: f32) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (door, far) = (DOOR / 2.0, length + x.abs());
    let boxes = [
        (
            "Front",
            (-far, -4.0 * HEIGHT, -WALL),
            (x - door, 4.0 * HEIGHT, WALL),
        ),
        (
            "Front",
            (x + door, -4.0 * HEIGHT, -WALL),
            (far, 4.0 * HEIGHT, WALL),
        ),
        (
            "Front",
            (x - door, 2.1, -WALL),
            (x + door, 4.0 * HEIGHT, WALL),
        ),
        (
            "Door",
            (x - door, -4.0 * HEIGHT, -WALL),
            (x + door, 0.0, WALL),
        ),
        (
            "Back",
            (-length / 2.0, 0.0, -5.2),
            (length / 2.0, HEIGHT, -5.0),
        ),
        ("Path", (x - 2.0, -0.4, WALL), (x + 2.0, -0.2, 22.0)),
    ];
    let ids: Vec<u32> = boxes
        .into_iter()
        .map(|(name, lo, hi)| block(&mut scene, name, lo.into(), hi.into()))
        .collect();
    (scene, ids[3], ids[4])
}
