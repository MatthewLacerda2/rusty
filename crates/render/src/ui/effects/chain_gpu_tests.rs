use glam::Vec4;
use rusty_scene::Scene;

use super::gpu_fixture::{canvas, element, ellipse, hidden_mask, Frames, SIZE};
use crate::Renderer;

const FULL: [f32; 4] = [0.0, 0.0, SIZE as f32, SIZE as f32];

fn child_mask(scene: &mut Scene, parent: u32) -> u32 {
    let child = element(scene, parent, FULL);
    hidden_mask(scene, child);
    let red = element(scene, child, FULL);
    scene.world.image_mut(red).expect("image").color = Vec4::new(1.0, 0.0, 0.0, 1.0);
    child
}

fn equals_a_fresh_draw(renderer: &mut Renderer, frames: &mut Frames, scene: &Scene, why: &str) {
    let (_, kept) = frames.frame(renderer, scene);
    let (_, fresh) = Frames::new(renderer).frame(renderer, scene);
    assert!(kept == fresh, "{why}: the child shows its old chain's cut");
}

#[test]
fn gpu_a_mask_added_or_removed_between_two_still_masks_redraws_the_child() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
    let (mut scene, root) = canvas();
    let p = element(&mut scene, root, FULL);
    hidden_mask(&mut scene, p);
    let m = element(&mut scene, p, FULL);
    ellipse(&mut scene, m);
    child_mask(&mut scene, m);
    let mut frames = Frames::new(&renderer);
    for _ in 0..2 {
        frames.frame(&mut renderer, &scene);
    }
    hidden_mask(&mut scene, m);
    equals_a_fresh_draw(&mut renderer, &mut frames, &scene, "M given a Mask");
    frames.frame(&mut renderer, &scene);
    scene.world.set_mask(m, None);
    equals_a_fresh_draw(&mut renderer, &mut frames, &scene, "M's Mask taken");
}

#[test]
fn gpu_a_mask_moved_between_two_still_sibling_masks_redraws() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
    let (mut scene, root) = canvas();
    let a = element(&mut scene, root, FULL);
    ellipse(&mut scene, a);
    hidden_mask(&mut scene, a);
    let b = element(&mut scene, root, FULL);
    hidden_mask(&mut scene, b);
    let c = child_mask(&mut scene, a);
    let mut frames = Frames::new(&renderer);
    for _ in 0..2 {
        frames.frame(&mut renderer, &scene);
    }
    scene.set_parent(c, Some(b)).expect("b exists");
    equals_a_fresh_draw(&mut renderer, &mut frames, &scene, "C moved from A to B");
    frames.frame(&mut renderer, &scene);
    scene.set_parent(c, Some(a)).expect("a exists");
    equals_a_fresh_draw(&mut renderer, &mut frames, &scene, "and back");
}
