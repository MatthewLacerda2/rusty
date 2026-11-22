use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::recipe::{arity, choice, op, opt, req, Lit, OpInfo, OpParam};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlopeMode {
    #[default]
    Blur,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlopeBlur {
    pub intensity: f32,
    #[serde(default = "samples")]
    pub samples: u32,
    #[serde(default)]
    pub mode: SlopeMode,
}

impl Row for SlopeBlur {
    const INFO: OpInfo = op(
        "slope_blur",
        "filter",
        arity(2, 2),
        &[
            req("intensity", "number"),
            opt("samples", "integer", Lit::Integer(SAMPLES)),
            OpParam {
                default: Some(Lit::Text("blur")),
                ..choice("mode", &["blur", "min", "max"])
            },
        ],
    );
}

impl Op for SlopeBlur {
    fn run(&self, cx: &Cx) -> Image {
        let p = self;
        slope_blur(cx.input(0), cx.input(1), p.intensity, p.samples, p.mode)
    }
}

const SAMPLES: u32 = 8;
pub const MAX_SAMPLES: u32 = 64;
const MAX_REACH: f32 = 1.0;
pub const SPAN: f32 = 1.0 / 128.0;
pub const MAX_INTENSITY: f32 = 1e6;

fn samples() -> u32 {
    SAMPLES
}

pub fn slope_blur(img: &Image, slope: &Image, intensity: f32, n: u32, mode: SlopeMode) -> Image {
    if intensity.is_nan() || intensity == 0.0 {
        return img.clone();
    }
    let intensity = intensity.clamp(-MAX_INTENSITY, MAX_INTENSITY);
    let n = n.clamp(1, MAX_SAMPLES);
    let (w, h) = (img.width() as f32, img.height() as f32);
    Image::fill_uv(img.size(), |u, v| {
        let (x, y) = ((u * w) as i64, (v * h) as i64);
        let s = |du, dv| slope.sample_bilinear_wrapped(u + du, v + dv)[0];
        let grad = (
            (s(SPAN, 0.0) - s(-SPAN, 0.0)) / (2.0 * SPAN),
            (s(0.0, SPAN) - s(0.0, -SPAN)) / (2.0 * SPAN),
        );
        let (du, dv) = reach(intensity, grad);
        let here = img.get_wrapped(x, y);
        if du == 0.0 && dv == 0.0 {
            return here;
        }
        let mut acc = here;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let p = img.sample_bilinear_wrapped(u + du * t, v + dv * t);
            for c in 0..4 {
                acc[c] = match mode {
                    SlopeMode::Blur => acc[c] + p[c],
                    SlopeMode::Min => acc[c].min(p[c]),
                    SlopeMode::Max => acc[c].max(p[c]),
                };
            }
        }
        match mode {
            SlopeMode::Blur => acc.map(|a| a / (n + 1) as f32),
            _ => acc,
        }
    })
}

fn reach(intensity: f32, (gu, gv): (f32, f32)) -> (f32, f32) {
    let (du, dv) = (intensity * gu, intensity * gv);
    let len = du.hypot(dv);
    if !len.is_finite() {
        (0.0, 0.0)
    } else if len > MAX_REACH {
        (du * MAX_REACH / len, dv * MAX_REACH / len)
    } else {
        (du, dv)
    }
}

#[cfg(test)]
#[path = "slope_blur_tests.rs"]
mod tests;
