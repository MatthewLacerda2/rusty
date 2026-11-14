//! The froxels stay within half-float (#1703): a thick medium at a bright lamp's
//! core holds far more than `Rgba16Float`'s 65504 per metre. Stored unclamped, it
//! is Inf, the integration's `Inf - Inf` drops that slice, and the lamp's core is a
//! dark hole. Clamped, every slice up to the lamp adds light and every texel is
//! finite.

use super::fixture::{draw, fog, lamp, renderer, room, view, LAMP_AT};
use crate::postfx::effects::volumetric_fog::VOLUME_NEAR;
use crate::readback::read_mip_bytes;
use rusty_components::{LightMode, VolumetricFog};
use rusty_core::color::f16_to_f32;

/// Slice `z` of the integrated volume's centre texel, after checking every
/// texel of the slice is finite.
fn centre_texel(r: &crate::Renderer, z: u32) -> [f32; 4] {
    let [w, h, _] = r.fog_volume.grid().expect("the volume is in use");
    let level = (r.fog_volume.texture(), 0, z);
    let bytes = read_mip_bytes(&r.device, &r.queue, level, (w, h), 8);
    let half = |at: usize| f16_to_f32(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
    let finite = (0..bytes.len()).step_by(2).all(|at| half(at).is_finite());
    assert!(finite, "slice {z} holds a non-finite texel");
    let at = (h as usize / 2 * w as usize + w as usize / 2) * 8;
    std::array::from_fn(|c| half(at + 2 * c))
}

#[test]
fn gpu_a_bright_lamp_in_thick_fog_stays_finite_and_lit() {
    let Some(mut r) = renderer() else { return };
    let thick = VolumetricFog {
        mean_free_path: VolumetricFog::MIN_MEAN_FREE_PATH,
        anisotropy: 0.9,
        ..fog()
    };
    let mut scene = room(thick);
    lamp(&mut scene, LAMP_AT, 200.0, LightMode::Realtime);
    let mut v = view(&r);
    for _ in 0..3 {
        draw(&mut r, &mut v, &scene);
    }
    let [_, _, d] = r.fog_volume.grid().expect("the volume is in use");
    let column: Vec<[f32; 4]> = (0..d).map(|z| centre_texel(&r, z)).collect();
    // `fog::volumetric::froxel_depth` past the first slice: texel z ends at z + 1.
    let far_end = |z: u32| {
        let ratio = (thick.depth_extent / VOLUME_NEAR).ln();
        VOLUME_NEAR * (ratio * (z + 1) as f32 / d as f32).exp()
    };
    let lamp_depth = -LAMP_AT.z;
    for z in 1..d {
        // In front of the lamp, where its light scatters forward to the eye (the
        // slices past it scatter backward, too little to show in a half float).
        if far_end(z) < lamp_depth - 1.5 || far_end(z) > lamp_depth {
            continue;
        }
        assert!(
            column[z as usize][0] > column[z as usize - 1][0],
            "slice {z} (to {} m) adds the lamp's light: {:?} after {:?}",
            far_end(z),
            column[z as usize],
            column[z as usize - 1]
        );
    }
}
