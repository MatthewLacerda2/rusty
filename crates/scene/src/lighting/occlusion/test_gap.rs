//! The long-wall-behind-a-door level the occlusion tests bake and render (#1714):
//! the case where a few points per object miss what a doorway shows.

use super::test_level::{block, DOOR, HEIGHT, WALL};
use crate::Scene;

/// A wall `length` long, 5 m behind a 1.4 m door at `x` in a wall that hides the
/// rest of it from an eye 20 m out (at `(x, 1.5, 20)`, looking down -Z, over a
/// path that puts it in the grid): the door's sill and the wall's ids.
pub fn gap(length: f32, x: f32) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (door, far) = (DOOR / 2.0, length + x.abs());
    // The front wall reaches far up and down, so no cell round the eye sees over or
    // under it: only the door shows the back wall.
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
