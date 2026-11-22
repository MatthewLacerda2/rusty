use super::super::{Grade, GradeCx, History, Owner, PostEffect};
use super::bounded;
use crate::postfx::params::Gate;
use rusty_components::{ColorAdjustments, Tonemap};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "color_adjustments",
    owner: Owner::Volume,
    gate: Gate::NONE,
    history: History::None("a grade of this frame's colour"),
    module: Some("postfx/effects/color_adjustments.wgsl"),
    composite: &[],
    grade: &[
        Grade {
            order: 15,
            function: "contrast",
            slots: 1,
            pack: pack_contrast,
        },
        Grade {
            order: 30,
            function: "hue",
            slots: 1,
            pack: pack_hue,
        },
        Grade {
            order: 35,
            function: "saturation",
            slots: 1,
            pack: pack_saturation,
        },
    ],
    passes: &[],
};

fn adjustments(cx: &GradeCx) -> ColorAdjustments {
    cx.volume.map(|vc| vc.color_adjustments).unwrap_or_default()
}

fn unit_range(v: f32) -> f32 {
    1.0 + bounded(v - 1.0, ColorAdjustments::MAX - 1.0)
}

fn pack_contrast(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    let mode = cx.volume.map_or(Tonemap::None, |vc| vc.tonemapping.mode);
    slots[0][0] = unit_range(adjustments(cx).contrast);
    slots[0][1] = mode.to_index() as f32;
}

fn pack_hue(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0][0] = bounded(adjustments(cx).hue_shift, 180.0) / 360.0;
}

fn pack_saturation(cx: &GradeCx, slots: &mut [[f32; 4]]) {
    slots[0][0] = unit_range(adjustments(cx).saturation);
}
