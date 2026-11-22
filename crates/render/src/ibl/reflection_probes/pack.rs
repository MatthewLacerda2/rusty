use super::MAX_REFLECTION_PROBES;
use rusty_scene::lighting::reflection_probe::ProbeBlend;

/// Unpacked by `environment_reflection` in `engine/shaders/forward/reflection.wgsl`: change
/// both together. 0 is the sky alone.
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
