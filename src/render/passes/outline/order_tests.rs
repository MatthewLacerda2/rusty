//! The outline and the gizmos draw in Unity's order (#1224): scene, post-FX, the
//! ring, then the handles. Post-FX never reads the ring or a gizmo as scene colour,
//! and an axis arrow leaving the silhouette is not painted over by the ring. Skips
//! with no adapter.

use glam::Vec3;

use super::tests::{is_outline, RES};
use crate::render::postfx::HDR_FORMAT;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A unit box at `(0, 1, 0)`, selected; off the grid's plane, so the grid stays out
/// of the rows the X arrow crosses.
fn selected_box() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = Vec3::Y;
    scene.selected_entity_id = Some(id);
    scene
}

/// The editor frame from 3 units in front of the box, as (RGBA8 output, the raw
/// HDR texels post-FX read).
fn render(r: &mut Renderer, scene: &Scene) -> (Vec<u8>, Vec<u8>) {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 3.0), -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, true);
    let output =
        readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), RES, RES);
    let texel = HDR_FORMAT.block_copy_size(None).unwrap();
    let hdr_texture = &view.post_fx.scene_hdr.texture;
    let hdr = readback::read_texture_bytes(&r.device, &r.queue, hdr_texture, (RES, RES), texel);
    (output, hdr)
}

#[test]
fn gpu_post_fx_reads_neither_the_ring_nor_the_gizmos() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = selected_box();
    let (selected, selected_hdr) = render(&mut r, &scene);
    scene.selected_entity_id = None;
    let (_, plain_hdr) = render(&mut r, &scene);
    let ring = selected.chunks(4).filter(|p| is_outline(p)).count();
    assert!(ring > 20, "the finished frame shows the ring, {ring} px");
    // Selecting adds the ring and the axis arrows; neither reaches the HDR scene.
    assert!(
        selected_hdr == plain_hdr,
        "post-FX's input changed when the box was selected"
    );
}

/// The X arrow's red: `[1.0, 0.1, 0.1]` on the sRGB target, nothing like the ring.
fn is_red_arrow(p: &[u8]) -> bool {
    p[0] > 180 && p[1] < 130 && p[2] > 50 && p[1].abs_diff(p[2]) < 24
}

#[test]
fn gpu_an_axis_arrow_keeps_its_colour_over_the_ring() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (px, _) = render(&mut r, &selected_box());
    let at = |x: u32, y: u32| &px[((y * RES + x) * 4) as usize..][..4];
    let mid = RES / 2;
    // The ring's columns right of the box, read on a row the arrow does not cross.
    let band: Vec<u32> = (mid..RES).filter(|&x| is_outline(at(x, mid - 6))).collect();
    assert!(!band.is_empty(), "the ring shows right of the box");
    // The X arrow leaves the silhouette along the middle rows, through that band.
    let red = (mid - 2..=mid + 2)
        .flat_map(|y| band.iter().map(move |&x| (x, y)))
        .filter(|&(x, y)| is_red_arrow(at(x, y)))
        .count();
    eprintln!("[arrow over ring] band {band:?}, {red} red px");
    assert!(
        red > 0,
        "the arrow shaft keeps its red where it crosses the ring"
    );
}
