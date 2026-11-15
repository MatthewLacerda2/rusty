//! What the radius field judges a hit's brightness against (#2249): the exposure
//! the frame is shown at, which the meter reads over the whole frame and eases.
//! The lamp-lit room ([`lit`]: its hits span dim walls to the bulb, whose
//! weights against each other depend on the reference), TAA off (no jitter, so a
//! still frame repeats), in Play (the meter eases), with a glowing patch; each
//! test reads the SSR-resolved frame outside the patch's columns, where only the
//! radius field can move it.

use std::ops::Range;

use glam::Vec3;

use super::super::super::fixture::place;
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer};
use rusty_components::{ExposureMode, MaterialAsset, MaterialComponent};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

use super::lit;

/// Seated, looking up at the ceiling, as `jitter` does, turned `turn` degrees.
pub(super) fn eye(turn: f32) -> Camera {
    Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0 + turn, 30.0)
}

/// A glowing box: where, its size, how bright.
pub(super) type Patch = (Vec3, Vec3, f32);

/// Where [`hanging`] hangs: under the ceiling 5 m out, in the frame's upper
/// rows (those the colour pyramid's 1×1 level covers, at 1080p).
pub(super) const HUNG: Vec3 = Vec3::new(0.0, 2.7, -5.0);

/// A patch 0.4 m across hung at `x`, glowing `glow`.
pub(super) fn hanging(x: f32, glow: f32) -> Patch {
    (HUNG + Vec3::X * x, Vec3::new(0.4, 0.4, 0.05), glow)
}

/// A panel glowing `glow` across the frame's lower left, a fifth of it: past
/// the meter's top tenth, so it moves the exposure.
pub(super) fn the_left_wall(glow: f32) -> Patch {
    (Vec3::new(-1.3, 2.0, -2.0), Vec3::new(1.4, 0.8, 0.05), glow)
}

/// [`lit`] at its own light, without TAA, in Play, exposed `mode` (`fixed_ev`
/// its fixed EV), and `patch`.
pub(super) fn scene((mode, fixed_ev): (ExposureMode, f32), (at, size, glow): Patch) -> Scene {
    let mut s = lit(1.0);
    s.set_playing(true);
    let cam = s.find_entity_by_name("Camera").unwrap();
    let mut camera = s.world.camera(cam).unwrap().clone();
    camera.taa_active = false;
    s.world.set_camera(cam, Some(camera));
    let mut volume = s.world.post_processing(cam).unwrap().clone();
    (volume.exposure.mode, volume.exposure.fixed_ev) = (mode, fixed_ev);
    s.world.set_post_processing(cam, Some(volume));
    let patch = create_entity(&mut s, "Patch", Some(Primitive::Box));
    place(&mut s, patch, at, size);
    let lamp = MaterialAsset {
        emissive: [glow; 3],
        ..MaterialAsset::default()
    };
    s.materials.insert("Patch".into(), lamp);
    s.world
        .set_material(patch, Some(MaterialComponent::new("Patch")));
    s
}

/// The world `x` at [`HUNG`] that lands on pixel column `px` of a `w × h` frame
/// through [`eye`].
pub(super) fn wall_x(px: f32, (w, h): (u32, u32), turn: f32) -> f32 {
    let vp = eye(turn).build_view_projection(w as f32 / h as f32);
    let col = |x: f32| {
        let c = vp * (HUNG + Vec3::X * x).extend(1.0);
        (c.x / c.w * 0.5 + 0.5) * w as f32
    };
    let (a, b) = (col(0.0), col(1.0));
    (px - a) / (b - a)
}

/// The SSR-resolved image of `v`'s last frame, in 255ths of perceptual luma,
/// over every column `x` outside `skip` (the patch and its reflection above it).
pub(super) fn outside(
    r: &Renderer,
    v: &RenderView,
    (w, h): (u32, u32),
    skip: Range<u32>,
) -> Vec<f32> {
    let bytes = read_texture_bytes(&r.device, &r.queue, &v.post_fx.scene_hdr.texture, (w, h), 8);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
    let at = |x: u32, y: u32| {
        let i = ((y * w + x) * 8) as usize;
        let l = 0.25 * half(i) + 0.5 * half(i + 2) + 0.25 * half(i + 4);
        255.0 * l / (1.0 + l)
    };
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|(x, _)| !skip.contains(x))
        .map(|(x, y)| at(x, y))
        .collect()
}

/// The largest per-pixel difference between two reads.
pub(super) fn apart(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
}

/// Draw `scene` into `v` through [`eye`] turned `turn`, at game time `frame`
/// sixtieths of a second (the meter eases on game time), and wait for its
/// exposure's readout.
pub(super) fn draw(
    r: &mut Renderer,
    v: &mut RenderView,
    (scene, turn): (&mut Scene, f32),
    frame: u32,
) -> Option<f32> {
    scene.shader_time = frame as f32 / 60.0;
    let target = v.color_target_view().unwrap();
    r.render(v, scene, &eye(turn), &target, false);
    v.settled_exposure(&r.device)
}

pub(super) const AUTO: (ExposureMode, f32) = (ExposureMode::Automatic, 0.0);
