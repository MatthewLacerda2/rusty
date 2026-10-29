//! src/procgen/image_buf.rs — the linear-RGBA pixel buffer the op-runner works in.
//!
//! Every op reads and writes [`Image`], a `width × height` grid of linear
//! `[f32; 4]` RGBA pixels. Working in unclamped linear float (not 8-bit) keeps
//! the op chain free of rounding until the single encode-on-bake step, and lets a
//! "scalar" value ride all four channels so a height field or mask is just an image.
//!
//! The domain **wraps**: index math is modular, so an op that samples a neighbour (a
//! blur, a bump-to-normal) reads across the edge seamlessly. That wrap is *why* a
//! baked map tiles — it is seamless by construction, not by a post-pass.
//!
//! The domain is the **unit square in relative units** whatever the image's shape,
//! the way Substance Designer works (#1493): pixel `(x, y)` of a `w × h` image sits
//! at `u = (x + ½) / w`, `v = (y + ½) / h`, so a non-square image samples the same
//! tile with more pixels along one axis, and the pattern stretches. An op with a
//! reach in tile units turns it into pixels per axis (`radius × w` across,
//! `radius × h` down).

/// One linear RGBA pixel. Unclamped on purpose: intermediate ops may over/undershoot
/// `[0, 1]`; the bake clamps and encodes once at the end.
pub type Rgba = [f32; 4];

/// An image's extent in pixels. A bare `u32` converts to the square of that side,
/// so square callers write `Image::new(64)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    /// A `width × height` extent.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// The pixel count, `width × height`.
    pub fn pixels(self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }

    /// The longer side.
    pub fn longer(self) -> u32 {
        self.width.max(self.height)
    }

    /// The extent with its sides swapped.
    pub fn transposed(self) -> Self {
        Self::new(self.height, self.width)
    }
}

impl From<u32> for Size {
    fn from(side: u32) -> Self {
        Self::new(side, side)
    }
}

/// A procedural image: `width × height` linear-RGBA pixels in row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    size: Size,
    pixels: Vec<Rgba>,
}

impl Image {
    /// A new image of `size` pixels, all `[0, 0, 0, 1]` (opaque black).
    pub fn new(size: impl Into<Size>) -> Self {
        Self::filled(size, [0.0, 0.0, 0.0, 1.0])
    }

    /// A new image filled with one constant pixel value.
    pub fn filled(size: impl Into<Size>, value: Rgba) -> Self {
        let size = size.into();
        Self {
            size,
            pixels: vec![value; size.pixels()],
        }
    }

    /// The image's extent in pixels.
    #[inline]
    pub fn size(&self) -> Size {
        self.size
    }

    /// Pixels per row.
    #[inline]
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// Rows.
    #[inline]
    pub fn height(&self) -> u32 {
        self.size.height
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
        let xi = x.rem_euclid(self.size.width as i64) as usize;
        let yi = y.rem_euclid(self.size.height as i64) as usize;
        self.pixels[yi * self.size.width as usize + xi]
    }

    /// Sample at a continuous domain point `(u, v)` (1.0 = one tile) with bilinear
    /// filtering between the four nearest pixel centers, wrapping at the edges — the
    /// resampler a domain transform uses so it is neither blocky nor seamed.
    pub fn sample_bilinear_wrapped(&self, u: f32, v: f32) -> Rgba {
        let (w, h) = (self.size.width as f32, self.size.height as f32);
        let (x, y) = (u * w - 0.5, v * h - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let (p00, p10) = (self.get_wrapped(xi, yi), self.get_wrapped(xi + 1, yi));
        let (p01, p11) = (
            self.get_wrapped(xi, yi + 1),
            self.get_wrapped(xi + 1, yi + 1),
        );
        std::array::from_fn(|c| {
            let top = p00[c] + (p10[c] - p00[c]) * tx;
            let bottom = p01[c] + (p11[c] - p01[c]) * tx;
            top + (bottom - top) * ty
        })
    }

    /// Row `y` of the image, left to right.
    #[inline]
    pub fn row(&self, y: usize) -> &[Rgba] {
        let w = self.size.width as usize;
        &self.pixels[y * w..(y + 1) * w]
    }

    /// Call `f(y, row)` on every row, the rows split into bands run in parallel
    /// ([`super::bands`]). Byte-identical to a serial loop as long as `f` writes
    /// `row` from its inputs and `y` alone — the shape of every op.
    pub fn for_each_row(&mut self, f: impl Fn(usize, &mut [Rgba]) + Sync) {
        super::bands::for_each_row(&mut self.pixels, self.size.width as usize, f);
    }

    /// Apply `f(u, v) -> Rgba` over the unit domain, where `u`/`v` are pixel-center
    /// coordinates in `[0, 1)`, each axis divided by its own pixel count. This is the
    /// generator entry point: an op that has no inputs paints itself by sampling its
    /// parametric function per pixel.
    pub fn fill_uv(size: impl Into<Size>, f: impl Fn(f32, f32) -> Rgba + Sync) -> Self {
        let mut img = Image::new(size);
        let (w, h) = (img.width() as f32, img.height() as f32);
        img.for_each_row(|y, row| {
            let v = (y as f32 + 0.5) / h;
            for (x, p) in row.iter_mut().enumerate() {
                *p = f((x as f32 + 0.5) / w, v);
            }
        });
        img
    }

    /// Map every pixel through `f` in place — the workhorse for per-pixel color/math
    /// ops that need no neighbourhood.
    pub fn map_in_place(&mut self, f: impl Fn(Rgba) -> Rgba + Sync) {
        self.for_each_row(|_, row| {
            for p in row {
                *p = f(*p);
            }
        });
    }

    /// The image mirrored about its diagonal: pixel `(x, y)` moves to `(y, x)`, and a
    /// `w × h` image becomes `h × w`. Lets a column pass run as a row pass, so it
    /// bands like every other op.
    pub fn transposed(&self) -> Image {
        let w = self.size.width as usize;
        let mut out = Image::new(self.size.transposed());
        out.for_each_row(|y, row| {
            for (x, p) in row.iter_mut().enumerate() {
                *p = self.pixels[x * w + y];
            }
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_image_is_opaque_black() {
        let img = Image::new(4);
        assert_eq!(img.size(), Size::new(4, 4));
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
    fn bilinear_sample_blends_neighbours_and_wraps() {
        let mut img = Image::new(2);
        img.pixels_mut()[0] = [1.0, 1.0, 1.0, 1.0]; // (0,0) white, (1,0) black
                                                    // A pixel center reads exactly its pixel.
        assert_eq!(
            img.sample_bilinear_wrapped(0.25, 0.25),
            [1.0, 1.0, 1.0, 1.0]
        );
        // Halfway between (0,0) and (1,0) on row 0 reads the average...
        assert_eq!(img.sample_bilinear_wrapped(0.5, 0.25)[0], 0.5);
        // ...and so does the wrap between (1,0) and (0,0).
        assert_eq!(img.sample_bilinear_wrapped(1.0, 0.25)[0], 0.5);
        assert_eq!(img.sample_bilinear_wrapped(0.0, 0.25)[0], 0.5);
    }

    #[test]
    fn fill_uv_samples_pixel_centers() {
        let img = Image::fill_uv(2, |u, _v| [u, 0.0, 0.0, 1.0]);
        // x=0 center is 0.25, x=1 center is 0.75 (res 2).
        assert_eq!(img.pixels()[0][0], 0.25);
        assert_eq!(img.pixels()[1][0], 0.75);
    }

    #[test]
    fn transposed_swaps_x_and_y() {
        let img = Image::fill_uv(3, |u, v| [u, v, 0.0, 1.0]);
        let t = img.transposed();
        for (x, y) in [(0, 1), (2, 0), (1, 2)] {
            let (p, q) = (img.get_wrapped(x, y), t.get_wrapped(y, x));
            assert_eq!((p[0], p[1]), (q[0], q[1]));
        }
        assert_eq!(t.transposed(), img);
    }

    #[test]
    fn a_wide_image_wraps_and_samples_per_axis() {
        let img = Image::fill_uv(Size::new(4, 2), |u, v| [u, v, 0.0, 1.0]);
        assert_eq!((img.width(), img.height(), img.pixels().len()), (4, 2, 8));
        // Pixel centres divide by each axis' own count.
        assert_eq!(img.get_wrapped(1, 1)[..2], [0.375, 0.75]);
        assert_eq!(img.get_wrapped(5, -1), img.get_wrapped(1, 1));
        assert_eq!(
            img.sample_bilinear_wrapped(0.375, 0.75),
            img.get_wrapped(1, 1)
        );
        let t = img.transposed();
        assert_eq!(t.size(), Size::new(2, 4));
        assert_eq!(t.get_wrapped(1, 3), img.get_wrapped(3, 1));
        assert_eq!(t.transposed(), img);
    }
}
