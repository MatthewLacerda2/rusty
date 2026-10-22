//! The ring is a fixed width around whatever the selection covers on screen
//! (#1175): a box stretched (10,1,1), a primitive level's usual wall, rings as wide
//! along its long sides as its short ones, and a glass box rings its silhouette
//! instead of filling it. A resized view rebuilds its mask at the new size. Skips
//! with no adapter.

use glam::Vec3;

use super::tests::{frame_from, is_outline, outline_stats, RES};
use super::OUTLINE_PX;
use crate::components::material::RenderMode;
use crate::components::{MaterialAsset, MaterialComponent};
use crate::render::{readback, RenderView, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A unit box at the origin, scaled by `scale`, selected.
fn selected_box(scale: Vec3) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().scale = scale;
    scene.selected_entity_id = Some(id);
    scene
}

/// Outline pixels on each side of the frame's centre along its middle row and
/// column: `[left, right, above, below]`.
fn ring_per_side(px: &[u8]) -> [usize; 4] {
    let half = (RES / 2) as usize;
    let hit = |x: usize, y: usize| is_outline(&px[(y * RES as usize + x) * 4..][..4]);
    let row = |xs: std::ops::Range<usize>| xs.filter(|&x| hit(x, half)).count();
    let col = |ys: std::ops::Range<usize>| ys.filter(|&y| hit(half, y)).count();
    let all = RES as usize;
    [row(0..half), row(half..all), col(0..half), col(half..all)]
}

#[test]
fn gpu_a_stretched_box_rings_evenly_on_all_four_sides() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    // About 48 px wide and 5 px tall from 16 m away.
    let scene = selected_box(Vec3::new(10.0, 1.0, 1.0));
    let sides = ring_per_side(&frame_from(&mut r, &scene, Vec3::ZERO, 16.0));
    eprintln!("[outline, stretched] left/right/above/below {sides:?} px");
    let width = OUTLINE_PX as usize;
    for (side, n) in ["left", "right", "above", "below"].iter().zip(sides) {
        assert!(
            (width - 1..=width + 1).contains(&n),
            "{side}: a {n} px ring, want about {width}"
        );
    }
}

#[test]
fn gpu_a_glass_box_rings_its_silhouette() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = selected_box(Vec3::ONE);
    let id = scene.selected_entity_id.unwrap();
    let material = MaterialComponent {
        material: "glass".to_string(),
    };
    scene.world.set_material(id, Some(material));
    let glass = MaterialAsset {
        render_mode: RenderMode::Transparent,
        ..MaterialAsset::default()
    };
    scene.materials.insert("glass".to_string(), glass);
    let (count, _, filled) = outline_stats(&frame_from(&mut r, &scene, Vec3::ZERO, 3.0));
    eprintln!("[outline, glass] {count} px");
    assert!(count > 20, "the glass box shows an outline");
    assert!(!filled, "the outline rings the glass, not fills it");
}

#[test]
fn gpu_a_resized_view_rebuilds_its_mask_and_still_rings() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = selected_box(Vec3::ONE);
    let cam = Camera::new(Vec3::Z * 3.0, -90.0, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    for (w, h) in [(RES, RES), (RES / 2, RES * 3 / 4)] {
        view.resize(&r.device, w, h, 2);
        let out = view.color_target_view().unwrap();
        r.render(&mut view, &scene, &cam, &out, true);
        let mask = view
            .selection_mask
            .as_ref()
            .expect("an outlined frame has a mask");
        assert!(mask.matches(w, h), "the mask is rebuilt at {w}x{h}");
        let px =
            readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), w, h);
        let ring = px.chunks(4).filter(|p| is_outline(p)).count();
        assert!(
            ring > 20,
            "{w}x{h}: the selection shows an outline ({ring} px)"
        );
    }
}
