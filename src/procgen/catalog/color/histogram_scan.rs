//! The `histogram_scan` op: a threshold that slides through the input's range
//! (Substance Designer's *Histogram Scan*, #1743).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::recipe::{op, opt, req, socket, Lit, OpInfo, Param, ONE};

/// A threshold per RGB channel: `position` slides it from all black (0) to all white
/// (1), and `contrast` sets its edge from the input's own gradient (0) to a hard step
/// (1). At `position = 0.5`, `contrast = 0` the input passes through. The one-knob
/// way to turn a noise into a mask of a chosen coverage. Alpha untouched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistogramScan {
    pub position: Param,
    #[serde(default)]
    pub contrast: Param,
}

impl Row for HistogramScan {
    const INFO: OpInfo = op(
        "histogram_scan",
        "color",
        ONE,
        &[
            socket(req("position", "number")),
            socket(opt("contrast", "number", Lit::Number(0.0))),
        ],
    );
}

impl Op for HistogramScan {
    fn run(&self, cx: &Cx) -> Image {
        let (position, contrast) = (cx.field(&self.position), cx.field(&self.contrast));
        let mut img = cx.own();
        img.map_indexed(|i, p| {
            let f = |c| scan(c, position.at(i), contrast.at(i));
            [f(p[0]), f(p[1]), f(p[2]), p[3]]
        });
        img
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("position", &self.position), ("contrast", &self.contrast)]
    }
}

/// One channel through the scan. The ramp is `1 − contrast` wide and its low end
/// sits at `1 − position × (1 + width)`, so it starts wholly above 1 (black) at
/// position 0 and ends wholly below 0 (white) at position 1, whatever the
/// contrast. Both knobs clamp to `[0, 1]`; at full contrast the ramp has no width,
/// and the scan is a hard step, white above `1 − position` (all of it at 1).
fn scan(c: f32, position: f32, contrast: f32) -> f32 {
    let (position, width) = (position.clamp(0.0, 1.0), 1.0 - contrast.clamp(0.0, 1.0));
    let low = 1.0 - position * (1.0 + width);
    if width < 1e-6 {
        f32::from(u8::from(c > low || position >= 1.0))
    } else {
        ((c - low) / width).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
#[path = "histogram_scan_tests.rs"]
mod tests;
