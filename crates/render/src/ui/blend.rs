use rusty_components::UiBlend;

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
