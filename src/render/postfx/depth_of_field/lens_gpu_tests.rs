//! The Bokeh lens at 480 × 360 (#1445), where the largest circle is 9 px and the
//! gather's rings at full reach sit 3 px apart: racking the aperture grows the blur
//! smoothly from the first pixel of CoC, and the focus is a plane, so a wall at the
//! focus distance under a wide lens is sharp to its corners.

use glam::Vec3;

use super::gpu_fixture::{bokeh, cube, eye, scene, FAR};
use super::DofParams;
use crate::components::{DepthOfField, MaterialComponent};
use crate::core::settings::quality::QualityPreset;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::camera::DEFAULT_FOV;
use crate::scene::Scene;

const W: u32 = 480;
const H: u32 = 360;

fn shot(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = v.color_target_view().unwrap();
    r.render(&mut v, scene, &eye(), &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, v.color_target().unwrap(), W, H)
}

/// Where camera-space point `p` lands through a vertical `fov` (degrees).
fn pixel(p: Vec3, fov: f32) -> (i32, i32) {
    let half = (fov.to_radians() * 0.5).tan() * -p.z;
    let ndc = (p.x / (half * W as f32 / H as f32), p.y / half);
    let x = (ndc.0 * 0.5 + 0.5) * W as f32;
    (x as i32, ((0.5 - ndc.1 * 0.5) * H as f32) as i32)
}

/// The largest channel difference between two frames within `reach` of `(cx, cy)`.
fn change(a: &[u8], b: &[u8], (cx, cy): (i32, i32), reach: i32) -> u8 {
    let rows = (cy - reach).max(0)..(cy + reach).min(H as i32);
    let at = |x: i32, y: i32| ((y as u32 * W + x as u32) * 4) as usize;
    rows.flat_map(|y| ((cx - reach).max(0)..(cx + reach).min(W as i32)).map(move |x| at(x, y)))
        .map(|i| (0..3).map(|c| a[i + c].abs_diff(b[i + c])).max().unwrap())
        .max()
        .unwrap_or(0)
}

/// How many pixels of row `y` within `reach` of column `x` the blur changed.
fn spread(a: &[u8], b: &[u8], (x, y): (i32, i32), reach: i32) -> i32 {
    let i = |x: i32| ((y as u32 * W + x as u32) * 4) as usize;
    let changed = |x| (0..3).any(|c| a[i(x) + c].abs_diff(b[i(x) + c]) > 6);
    (x - reach..=x + reach).filter(|&x| changed(x)).count() as i32
}

#[test]
fn gpu_bokeh_blur_grows_smoothly_with_the_circle_of_confusion() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    r.quality = QualityPreset::High;
    let sharp = shot(&mut r, &scene(DepthOfField::default()).0);
    // The far cube's left edge, on its front face (1.5 m wide, 11.25 m out).
    let face = FAR + Vec3::new(-0.75, 0.0, 0.75);
    let edge = pixel(face, DEFAULT_FOV);
    let mut last = (0.0, 0);
    for f_stop in [16.0, 11.0, 8.0, 6.4, 5.3, 4.5, 3.9, 3.4, 3.0, 2.6] {
        let dof = DepthOfField {
            aperture: f_stop,
            ..bokeh(2.0)
        };
        let coc = DofParams::new(&dof, H, 1).unwrap().coc(-face.z);
        let width = spread(&shot(&mut r, &scene(dof).0), &sharp, edge, 16);
        eprintln!("[dof] f/{f_stop}: CoC {coc:.2} px, edge spread {width} px");
        assert!(width >= 1, "a {coc:.2} px blur is covered ({width})");
        assert!(
            width + 1 >= last.1,
            "the blur never shrinks ({last:?} → {width})"
        );
        let step = (coc - last.0) as i32;
        assert!(
            width - last.1 <= 2 * step + 4,
            "no jump ({last:?} → {width})"
        );
        last = (coc, width);
    }
    assert!(last.1 >= 8, "the widest blur spreads ({})", last.1);
}

/// The two-cube scene under a 100° lens, with a dark wall 3 m out filling the view
/// and a bright tile in each corner just in front of it.
fn wide_wall(dof: DepthOfField) -> (Scene, Vec<Vec3>) {
    let (mut scene, cam) = scene(dof);
    scene.world.camera_mut(cam).unwrap().fov = 100.0;
    let wall = cube(&mut scene, Vec3::new(0.0, 0.0, -3.0));
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(20.0, 20.0, 0.02);
    let dark = Some(MaterialComponent::new("wall".to_string()));
    scene.world.set_material(wall, dark);
    let corners: Vec<Vec3> = [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .map(|(x, y)| Vec3::new(3.8 * x, 2.8 * y, -2.98))
        .into();
    for &at in &corners {
        let tile = cube(&mut scene, at);
        scene.world.transform_mut(tile).unwrap().scale = Vec3::new(0.6, 0.6, 0.02);
    }
    (scene, corners)
}

#[test]
fn gpu_a_wall_at_the_focus_distance_stays_sharp_to_its_corners() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    r.quality = QualityPreset::High;
    let (off, corners) = wide_wall(DepthOfField::default());
    let off = shot(&mut r, &off);
    // 5.6 m away in a straight line, the corners sit 3 m deep, in focus.
    let focused = shot(&mut r, &wide_wall(bokeh(3.0)).0);
    let missed = shot(&mut r, &wide_wall(bokeh(6.0)).0);
    for at in corners {
        let p = pixel(at, 100.0);
        let (kept, blurred) = (change(&focused, &off, p, 14), change(&missed, &off, p, 14));
        eprintln!("[dof] corner {at}: focused changed {kept}, focus 6 m {blurred}");
        assert!(
            kept <= 4,
            "a corner on the focus plane stays sharp ({kept})"
        );
        assert!(blurred >= 16, "out of focus the corner blurs ({blurred})");
    }
}
