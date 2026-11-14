use glam::{Vec2, Vec3};

use super::*;
use rusty_navigation::Surface;
use rusty_render::{readback, RenderView, OFFSCREEN_FORMAT};
use rusty_scene::{Camera, Scene};

/// A one-cell graph (1 m cells) whose one span is a 0.3-grade ramp rising along +z,
/// its top at 0.15 (the uphill edge).
fn ramp_cell() -> NavigationGraph {
    let mut g = NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 1.0);
    g.spans[0].y = 0.15;
    g.spans[0].surface = Surface {
        centre: -0.15,
        slope: Vec2::new(0.0, 0.3),
        floor: f32::INFINITY,
    };
    g
}

/// One span is one quad (two triangles) covering its cell, on its floor plane.
#[test]
fn one_span_is_one_quad_on_its_plane() {
    let tris = tile_triangles(&ramp_cell(), 0);
    assert_eq!(tris.len(), 6, "two triangles");
    for p in &tris {
        assert!(p.x.abs() == 0.5 && p.z.abs() == 0.5, "a cell corner: {p}");
        // The plane: 0.15 at the uphill edge (z = 0.5), 0.3 lower at the downhill one.
        let want = 0.15 + 0.3 * (p.z - 0.5);
        assert!((p.y - LIFT - want).abs() < 1e-5, "on the plane: {p}");
    }
    let low = tris
        .iter()
        .filter(|p| p.z < 0.0)
        .all(|p| (p.y - LIFT + 0.15).abs() < 1e-5);
    let high = tris
        .iter()
        .filter(|p| p.z > 0.0)
        .all(|p| (p.y - LIFT - 0.15).abs() < 1e-5);
    assert!(low && high, "the ramp tilts: {tris:?}");
}

/// A span of the not-walkable area draws nothing.
#[test]
fn a_not_walkable_span_gives_no_quad() {
    let mut g = ramp_cell();
    g.spans[0].area = NOT_WALKABLE_AREA;
    assert!(tile_triangles(&g, 0).is_empty());
}

/// A Dust2-sized graph (200 m square at 0.25 m cells: 641 601 spans) is built and
/// uploaded once; a frame on which it did not change uploads nothing, and another
/// graph of the same shape and generation (a wholesale replacement) is new.
#[test]
fn gpu_an_unchanged_dust2_graph_is_not_rebuilt() {
    let Some(mut r) = rusty_render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let g = NavigationGraph::new(-100.0, 100.0, -100.0, 100.0, 0.25);
    let mut on = SceneGizmos::default();
    assert_eq!(hand(&mut r, &on, &g), 0, "off: nothing built");
    on.set(Gizmo::NavMesh, true);
    let whole = 641_601 * 6 * 12;
    assert_eq!(
        hand(&mut r, &on, &g),
        whole,
        "the first frame builds it all"
    );
    assert_eq!(hand(&mut r, &on, &g), 0, "an unchanged frame does not");
    let other = Box::new(NavigationGraph::new(-100.0, 100.0, -100.0, 100.0, 0.25));
    assert_eq!(hand(&mut r, &on, &other), whole, "another graph does");
}

const RES: u32 = 32;

/// A pixel of the editor frame looking straight down at a flat navmesh, a quarter
/// in from a corner: off the floor grid's centre lines, which draw over it.
fn sample(r: &mut Renderer, gizmos: SceneGizmos) -> [u8; 4] {
    let g = NavigationGraph::new(-2.0, 2.0, -2.0, 2.0, 0.5);
    hand(r, &gizmos, &g);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    view.gizmos = gizmos;
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 3.0, 0.001), -90.0, -89.0);
    r.render(&mut view, &Scene::new(), &cam, &out, true);
    let px =
        readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), RES, RES);
    let i = ((RES / 4) * RES + RES / 4) as usize * 4;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

/// With the gizmo on, the floor under the navmesh turns cyan; off, it is untouched.
#[test]
fn gpu_the_navmesh_tints_the_floor_cyan_under_its_toggle() {
    let Some(mut r) = rusty_render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let off = sample(&mut r, SceneGizmos::default());
    let mut set = SceneGizmos::default();
    set.set(Gizmo::NavMesh, true);
    let on = sample(&mut r, set);
    assert_ne!(on, off, "the toggle shows it");
    assert!(on[2] > on[0] && on[1] > on[0], "cyan: {on:?} over {off:?}");
}
