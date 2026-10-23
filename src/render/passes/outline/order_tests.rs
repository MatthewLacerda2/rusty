//! The outline and the gizmos draw in Unity's order (#1224): scene, post-FX, the
//! ring, then the handles. Post-FX never reads the ring or a gizmo as scene colour,
//! and an axis arrow leaving the silhouette is not painted over by the ring. Skips
//! with no adapter.

use glam::Vec3;

use super::tests::{is_outline, RES};
use crate::render::postfx::HDR_FORMAT;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::lighting::probe::Probe;
use crate::scene::{Camera, Scene};

/// A unit box at `(0, 1, 0)`, selected, beside a light probe whose marker is in
/// view; off the grid's plane, so the grid stays out of the rows the X arrow crosses.
fn selected_box() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = Vec3::Y;
    scene.selected_entity_id = Some(id);
    let probe = Probe::new(Vec3::new(-0.8, 1.6, 0.6));
    scene.probes.probes.push(probe);
    scene
}

/// The frame from 3 units in front of the box, editor or game (`editor`), as
/// (RGBA8 output, the raw HDR texels post-FX read).
fn render(r: &mut Renderer, scene: &Scene, editor: bool) -> (Vec<u8>, Vec<u8>) {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 3.0), -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, editor);
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
    let scene = selected_box();
    let (editor, editor_hdr) = render(&mut r, &scene, true);
    let (_, game_hdr) = render(&mut r, &scene, false);
    let ring = editor.chunks(4).filter(|p| is_outline(p)).count();
    assert!(ring > 20, "the finished frame shows the ring, {ring} px");
    // The editor adds the ring, the axis arrows, the grid and the probe's marker;
    // none of them reaches the HDR scene a game frame would post-process.
    assert!(
        editor_hdr == game_hdr,
        "post-FX's input differs between the editor and a game frame"
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
    let (px, _) = render(&mut r, &selected_box(), true);
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
