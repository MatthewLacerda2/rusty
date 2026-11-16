//! What the march tests (#2228) share: one frame's raw SSR hits read back, and
//! where a colour lies in them.

use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_core::color::f16_to_f32;
use rusty_scene::{Camera, Scene};

/// One frame's trace hits (`postfx/ssr.wgsl`'s `Traced::hit`: colour times
/// weight), linear RGB per pixel, row by row, of a `w` × `h` view.
pub(super) struct Hits {
    pub(super) w: u32,
    pub(super) h: u32,
    rgb: Vec<[f32; 3]>,
}

impl Hits {
    /// Render `scene` through `cam` once into a fresh `w` × `h` view and read its hits.
    pub(super) fn of(r: &mut Renderer, scene: &Scene, cam: &Camera, (w, h): (u32, u32)) -> Self {
        let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, w, h);
        let out = view.color_target_view().unwrap();
        r.render(&mut view, scene, cam, &out, false);
        let texture = view.post_fx.ssr_hits().expect("SSR ran");
        let bytes = read_texture_bytes(&r.device, &r.queue, texture, (w, h), 8);
        let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
        let rgb = (0..(w * h) as usize)
            .map(|t| [0, 1, 2].map(|c| half(t * 8 + c * 2)))
            .collect();
        Self { w, h, rgb }
    }

    /// Pixel `(x, y)`'s hit colour.
    pub(super) fn at(&self, x: u32, y: u32) -> [f32; 3] {
        self.rgb[(y * self.w + x) as usize]
    }

    /// Whether `(x, y)` hit something mostly of channel `c` (0 red, 1 green):
    /// that channel at least twice each other one, and lit.
    pub(super) fn is(&self, x: u32, y: u32, c: usize) -> bool {
        let p = self.at(x, y);
        p[c] > 0.05 && (0..3).all(|o| o == c || p[c] > 2.0 * p[o])
    }

    /// How many pixels hit something mostly of channel `c`.
    pub(super) fn count(&self, c: usize) -> usize {
        (0..self.h)
            .flat_map(|y| (0..self.w).map(move |x| (x, y)))
            .filter(|&(x, y)| self.is(x, y, c))
            .count()
    }

    /// The rows in which some pixel hit something mostly of channel `c`.
    pub(super) fn rows(&self, c: usize) -> Vec<u32> {
        (0..self.h)
            .filter(|&y| (0..self.w).any(|x| self.is(x, y, c)))
            .collect()
    }
}
