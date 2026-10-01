//! src/render/ui/blend.rs — a `UiBlend` mode as GPU blend state (#425).
//!
//! The shader outputs premultiplied colour `s` with alpha `a`; under it is `d`.
//! Each mode is one fixed-function blend, so each is its own pipeline and a change
//! of mode breaks a batch:
//!
//! - `Normal` — `s + d(1 − a)` (over);
//! - `Additive` — `s + d`;
//! - `Multiply` — `s·d + d(1 − a)`: an opaque source multiplies, transparency fades
//!   it back to `d`;
//! - `Screen` — `s + d(1 − s)`.
//!
//! Alpha always composites "over", so coverage accumulates the same in every mode.

use crate::components::UiBlend;

/// The colour target blend for `mode`.
pub(crate) fn blend_state(mode: UiBlend) -> wgpu::BlendState {
    use wgpu::{BlendComponent, BlendFactor as F, BlendOperation};
    let color = |src_factor, dst_factor| BlendComponent {
        src_factor,
        dst_factor,
        operation: BlendOperation::Add,
    };
    let color = match mode {
        UiBlend::Normal => color(F::One, F::OneMinusSrcAlpha),
        UiBlend::Additive => color(F::One, F::One),
        UiBlend::Multiply => color(F::Dst, F::OneMinusSrcAlpha),
        UiBlend::Screen => color(F::One, F::OneMinusSrc),
    };
    wgpu::BlendState {
        color,
        alpha: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING.alpha,
    }
}

/// `mode`'s slot in a per-mode pipeline array (`UiBlend::ALL` order).
pub(crate) fn index(mode: UiBlend) -> usize {
    UiBlend::ALL.iter().position(|&b| b == mode).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_is_premultiplied_over_and_every_mode_has_a_slot() {
        assert_eq!(
            blend_state(UiBlend::Normal),
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING
        );
        let slots: Vec<usize> = UiBlend::ALL.into_iter().map(index).collect();
        assert_eq!(slots, vec![0, 1, 2, 3]);
    }
}
