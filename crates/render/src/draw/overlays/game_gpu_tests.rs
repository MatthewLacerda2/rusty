//! A game frame's gizmos on lavapipe (#1882): drawn through the base camera of an
//! FPS stack, against its depth; left out (without a crash) while TAA upscales.

use glam::Vec3;

use crate::motion::gpu_fixture::{cube, eye, renderer, scene, H, W};
use crate::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::CameraComponent;
use rusty_core::settings::render_scale::RenderScale;
use rusty_scene::{ClearFlags, Scene};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const MAGENTA: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

/// The fixture scene (TAA on when `taa`): a wide wall 6 m out, a red line in front
/// of it and a magenta one behind it, both held for a second.
fn lined(taa: bool) -> Scene {
    let (mut scene, cam) = scene(false);
    scene.world.camera_mut(cam).unwrap().taa_active = taa;
    let wall = cube(&mut scene, Vec3::new(0.0, 0.0, -6.0));
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(12.0, 12.0, 0.2);
    let lines = &mut scene.debug_lines;
    lines.push(
        Vec3::new(-1.0, 0.5, -4.0),
        Vec3::new(1.0, 0.5, -4.0),
        RED,
        60,
    );
    lines.push(
        Vec3::new(-1.0, -0.5, -9.0),
        Vec3::new(1.0, -0.5, -9.0),
        MAGENTA,
        60,
    );
    scene
}

/// One fresh frame of `scene` at `scale`, its game gizmos on or off.
fn shot(r: &mut Renderer, scene: &Scene, scale: f32, gizmos: bool) -> (Vec<u8>, RenderView) {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    v.set_render_scale(RenderScale::new(scale));
    v.resize(&r.device, W, H);
    v.game_gizmos = gizmos;
    let out = v.color_target_view().unwrap();
    r.render(&mut v, scene, &eye(0.0), &out, false);
    let target = v.color_target().unwrap();
    let img = readback::read_texture_rgba8(&r.device, &r.queue, target, W, H);
    (img, v)
}

/// The pixels holding `color` (its strong channels high, the rest low).
fn pixels_of(img: &[u8], color: [f32; 4]) -> Vec<usize> {
    let hit = |p: &[u8]| (0..3).all(|c| (p[c] > 180) == (color[c] > 0.5));
    img.chunks(4)
        .enumerate()
        .filter(|(_, p)| hit(p))
        .map(|(i, _)| i)
        .collect()
}

/// `scene` under a `DepthOnly` viewmodel camera with a narrow lens drawing nothing.
fn with_viewmodel(mut scene: Scene) -> Scene {
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        culling_mask: 0,
        fov: 20.0,
        near: 0.01,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    scene
}

/// Under a `DepthOnly` viewmodel camera with a narrow lens that draws nothing, the
/// gizmos land where the world camera alone draws them, and the line behind the
/// wall stays hidden though the viewmodel cleared the depth buffer.
#[test]
fn gpu_game_gizmos_follow_the_base_camera_under_a_viewmodel() {
    let Some(mut r) = renderer() else { return };
    let alone = lined(false);
    let (single, _) = shot(&mut r, &alone, 1.0, true);
    let red = pixels_of(&single, RED);
    assert!(red.len() >= 8, "the red line drew: {} pixels", red.len());
    assert!(
        pixels_of(&single, MAGENTA).is_empty(),
        "the wall hides the magenta"
    );

    let stacked = with_viewmodel(lined(false));
    let (under, mut v) = shot(&mut r, &stacked, 1.0, true);
    assert!(v.gizmo_depth.is_some(), "the base camera's depth is kept");
    v.game_gizmos = false;
    let out = v.color_target_view().unwrap();
    r.render(&mut v, &stacked, &eye(0.0), &out, false);
    assert!(v.gizmo_depth.is_none(), "freed once the gizmos are off");
    eprintln!(
        "[gizmos] red alone {}, under a viewmodel {}",
        red.len(),
        pixels_of(&under, RED).len()
    );
    assert_eq!(pixels_of(&under, RED), red, "the base camera's projection");
    assert!(
        pixels_of(&under, MAGENTA).is_empty(),
        "the base camera's depth"
    );
}

/// While TAA upscales the view (render scale 0.75) the gizmos are left out, the
/// console told once, the frame is the plain one and no depth is copied for them;
/// at a scale of 1 they draw.
#[test]
fn gpu_game_gizmos_under_taa_upscaling_are_skipped_not_a_crash() {
    let Some(mut r) = renderer() else { return };
    let scene = lined(true);
    let (plain, _) = shot(&mut r, &scene, 0.75, false);
    let (gizmos, v) = shot(&mut r, &scene, 0.75, true);
    assert!(v.gizmo_upscale_told, "the console was told");
    assert_eq!(gizmos, plain, "nothing drawn while TAA upscales");
    let (_, v) = shot(&mut r, &with_viewmodel(lined(true)), 0.75, true);
    assert!(v.gizmo_depth.is_none(), "no depth kept where nothing draws");
    let (native, _) = shot(&mut r, &scene, 1.0, true);
    assert!(
        !pixels_of(&native, RED).is_empty(),
        "TAA at scale 1 draws them"
    );
}
