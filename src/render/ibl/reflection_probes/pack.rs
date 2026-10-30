//! Packing an object's probe blend into the one `u32` its instance carries (#1584).

use super::MAX_REFLECTION_PROBES;
use crate::scene::lighting::reflection_probe::ProbeBlend;

/// The packed blend an instance carries in `InstanceData::reflection_probe`, which
/// `environment_reflection` in `engine/shaders/forward/lighting.wgsl` unpacks: bits
/// 0–5 the first probe's index plus one, 6–11 the second's (0 for none), 12–21 the
/// first's weight and 22–31 the second's, each in 1023ths; the sky takes what they
/// leave. 0 is "the sky alone".
pub(crate) fn pack_blend(blend: Option<ProbeBlend>) -> u32 {
    let Some(b) = blend else {
        return 0;
    };
    let weight = |w: f32| (w.clamp(0.0, 1.0) * 1023.0).round() as u32;
    let slot = |i: usize| (i as u32 + 1).min(MAX_REFLECTION_PROBES as u32);
    let second = b.second.map_or(0, slot);
    slot(b.first) | second << 6 | weight(b.first_weight) << 12 | weight(b.second_weight) << 22
}

#[cfg(test)]
#[path = "pack_tests.rs"]
mod pack_tests;
