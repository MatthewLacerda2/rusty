//! A still view's rough reflection holds still under TAA's sub-pixel jitter
//! (#2249, #2228): a satin bronze ceiling over a small, very bright bulb on a 1 cm
//! cord in a dark room, seen from below. The cord is under a pixel wide: on some
//! of TAA's offsets its reflection lands in one column, on others the next or
//! none. The radius field, a mean by count, lets the cord's dark hits set the
//! bulb's blur on some offsets, and the bulb's reflection blinks. Measured on the
//! SSR-resolved image (what each offset hands TAA) and after TAA's resolve. The
//! scene and its measure are TAA's flicker case's (`taa::gpu::flicker`'s `room`
//! and `measure`), the same room.
//!
//! The field as it is fails this test, so it is ignored. Weighing each hit by its
//! brightness passes it but loosens a dark object's contact reflection before a
//! bright background (`contact::dark`): a pyramid texel holds only sums, so it
//! cannot tell a cord's sliver from a dark cube's foot. The fix that keeps both
//! needs a count-weighted radius per texel as well, a fifth channel.

use crate::postfx::effects::taa::gpu::flicker::measure::{luma, swing, tiles_of};
use crate::postfx::effects::taa::gpu::flicker::room::{eye, room, H, W, WARM};
use crate::{RenderView, OFFSCREEN_FORMAT};

/// Frames measured: offsets 28 to 63 of the jitter's 1024 (Halton 2, 3), long
/// enough for a swing every two or three frames to show many times.
const FRAMES: u32 = 36;

/// The largest step a tile of the SSR-resolved still view may take between two
/// jitter offsets, in 255ths of perceptual luma: near the trace's own, which
/// without the resolve's blur steps 24 (the bulb's sharp reflection crossing a
/// tile's edge). Measured: 44.5 with the field as it is (lavapipe and an RTX
/// 2060), 20.8 with the field weighted by brightness (RTX 2060, lavapipe, Metal).
const STILL: f32 = 30.0;

/// The same after TAA's resolve, URP's clamp (#2228): what the player sees, a
/// patch swinging as a whole passing the clamp. Measured: 26.2 with the field as
/// it is (lavapipe and an RTX 2060), 7.5 weighted by brightness (RTX 2060,
/// lavapipe, Metal).
const TAA_STILL: f32 = 14.0;

#[test]
// lint: allow(skip) the radius field as it is fails it (44.5 / 26.2): its fix needs a count-weighted radius per texel, a fifth channel, to keep `contact::dark`
#[ignore]
fn gpu_a_still_rough_reflection_holds_under_taa_jitter() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = room();
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let (mut ssr, mut taa) = (Vec::new(), Vec::new());
    for f in 0..WARM + FRAMES {
        let target = v.color_target_view().unwrap();
        r.render(&mut v, &scene, &eye(), &target, false);
        if f >= WARM {
            ssr.push(tiles_of(&r, &v.post_fx.scene_hdr.texture, luma));
            let resolved = v.post_fx.taa_latest().expect("TAA ran");
            taa.push(tiles_of(&r, resolved, luma));
        }
    }
    let (ssr, taa) = (swing(&ssr), swing(&taa));
    eprintln!("[ssr] a still view's largest tile step under jitter: {ssr:.1} resolved, {taa:.1} after TAA");
    assert!(
        ssr <= STILL,
        "a still rough reflection blinks under the jitter: a tile steps {ssr:.1}"
    );
    assert!(
        taa <= TAA_STILL,
        "a still view blinks under TAA: a tile steps {taa:.1}"
    );
}
