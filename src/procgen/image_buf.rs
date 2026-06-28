//! src/procgen/image_buf.rs — the linear-RGBA pixel buffer the op-runner works in.
//!
//! Every op reads and writes [`Image`], a square `resolution × resolution` grid of
//! linear `[f32; 4]` RGBA pixels. Working in unclamped linear float (not 8-bit) keeps
//! the op chain free of rounding until the single encode-on-bake step, and lets a
//! "scalar" value ride all four channels so a height field or mask is just an image.
//!
//! The domain **wraps**: index math is modular, so an op that samples a neighbour (a
//! blur, a bump-to-normal) reads across the edge seamlessly. That wrap is *why* a
//! baked map tiles — it is seamless by construction, not by a post-pass.

/// One linear RGBA pixel. Unclamped on purpose: intermediate ops may over/undershoot
/// `[0, 1]`; the bake clamps and encodes once at the end.
pub type Rgba = [f32; 4];

/// A square procedural image: `resolution²` linear-RGBA pixels in row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    resolution: u32,
    pixels: Vec<Rgba>,
}

impl Image {
    /// A new image of `resolution²` pixels, all `[0, 0, 0, 1]` (opaque black).
    pub fn new(resolution: u32) -> Self {
        let n = (resolution as usize).saturating_mul(resolution as usize);
        Self {
            resolution,
            pixels: vec![[0.0, 0.0, 0.0, 1.0]; n],
        }
    }

    /// A new image filled with one constant pixel value.
    pub fn filled(resolution: u32, value: Rgba) -> Self {
        let n = (resolution as usize).saturating_mul(resolution as usize);
        Self {
            resolution,
            pixels: vec![value; n],
        }
    }

    /// The image's edge length in pixels (it is square).
    #[inline]
    pub fn resolution(&self) -> u32 {
        self.resolution
    }

    /// Read-only view of the row-major pixel slice.
    #[inline]
    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    /// Mutable view of the row-major pixel slice (op writes go here).
    #[inline]
    pub fn pixels_mut(&mut self) -> &mut [Rgba] {
        &mut self.pixels
    }

    /// Fetch a pixel by **wrapped** integer coordinate — `x`/`y` may be negative or
    /// past the edge and they fold back into range, so neighbour reads tile.
    #[inline]
    pub fn get_wrapped(&self, x: i64, y: i64) -> Rgba {
        let r = self.resolution as i64;
        let xi = x.rem_euclid(r) as usize;
        let yi = y.rem_euclid(r) as usize;
        self.pixels[yi * self.resolution as usize + xi]
    }

    /// Apply `f(u, v) -> Rgba` over the unit domain, where `u`/`v` are pixel-center
    /// coordinates in `[0, 1)`. This is the generator entry point: an op that has no
    /// inputs paints itself by sampling its parametric function per pixel.
    pub fn fill_uv(resolution: u32, mut f: impl FnMut(f32, f32) -> Rgba) -> Self {
        let mut img = Image::new(resolution);
        let res = resolution as f32;
        for y in 0..resolution {
            let v = (y as f32 + 0.5) / res;
            for x in 0..resolution {
                let u = (x as f32 + 0.5) / res;
                img.pixels[(y * resolution + x) as usize] = f(u, v);
            }
        }
        img
    }

    /// Map every pixel through `f` in place — the workhorse for per-pixel color/math
    /// ops that need no neighbourhood.
    pub fn map_in_place(&mut self, mut f: impl FnMut(Rgba) -> Rgba) {
        for p in &mut self.pixels {
            *p = f(*p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_image_is_opaque_black() {
        let img = Image::new(4);
        assert_eq!(img.resolution(), 4);
        assert_eq!(img.pixels().len(), 16);
        assert!(img.pixels().iter().all(|p| *p == [0.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn get_wrapped_folds_out_of_range_coords() {
        let mut img = Image::new(2);
        img.pixels_mut()[0] = [1.0, 0.0, 0.0, 1.0]; // (0,0)
        img.pixels_mut()[3] = [0.0, 1.0, 0.0, 1.0]; // (1,1)
        assert_eq!(img.get_wrapped(0, 0), img.get_wrapped(2, 2));
        assert_eq!(img.get_wrapped(0, 0), img.get_wrapped(-2, -2));
        assert_eq!(img.get_wrapped(1, 1), img.get_wrapped(-1, -1));
    }

    #[test]
    fn fill_uv_samples_pixel_centers() {
        let img = Image::fill_uv(2, |u, _v| [u, 0.0, 0.0, 1.0]);
        // x=0 center is 0.25, x=1 center is 0.75 (res 2).
        assert_eq!(img.pixels()[0][0], 0.25);
        assert_eq!(img.pixels()[1][0], 0.75);
    }
}
