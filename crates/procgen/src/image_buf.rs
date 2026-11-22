//! The domain is the **unit square in relative units** whatever the image's shape,
//! the way Substance Designer works: pixel `(x, y)` of a `w × h` image sits
//! at `u = (x + ½) / w`, `v = (y + ½) / h`, so a non-square image samples the same
//! tile with more pixels along one axis, and the pattern stretches. An op with a
//! reach in tile units turns it into pixels per axis (`radius × w` across,
//! `radius × h` down).

pub type Rgba = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn pixels(self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }

    pub fn longer(self) -> u32 {
        self.width.max(self.height)
    }

    pub fn transposed(self) -> Self {
        Self::new(self.height, self.width)
    }
}

impl From<u32> for Size {
    fn from(side: u32) -> Self {
        Self::new(side, side)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    size: Size,
    pixels: Vec<Rgba>,
}

impl Image {
    pub fn new(size: impl Into<Size>) -> Self {
        Self::filled(size, [0.0, 0.0, 0.0, 1.0])
    }

    pub fn filled(size: impl Into<Size>, value: Rgba) -> Self {
        let size = size.into();
        Self {
            size,
            pixels: vec![value; size.pixels()],
        }
    }

    #[inline]
    pub fn size(&self) -> Size {
        self.size
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.size.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.size.height
    }

    #[inline]
    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    #[inline]
    pub fn pixels_mut(&mut self) -> &mut [Rgba] {
        &mut self.pixels
    }

    #[inline]
    pub fn get_wrapped(&self, x: i64, y: i64) -> Rgba {
        let xi = x.rem_euclid(self.size.width as i64) as usize;
        let yi = y.rem_euclid(self.size.height as i64) as usize;
        self.pixels[yi * self.size.width as usize + xi]
    }

    pub fn sample_bilinear_wrapped(&self, u: f32, v: f32) -> Rgba {
        let (w, h) = (self.size.width as f32, self.size.height as f32);
        let finite = |a: f32| if a.is_finite() { a } else { 0.0 };
        let (x, y) = (finite(u * w - 0.5), finite(v * h - 0.5));
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        // Folded while still a float (exact for a whole number), so `+ 1` can't overflow.
        let (xi, yi) = (x0.rem_euclid(w) as i64, y0.rem_euclid(h) as i64);
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

    #[inline]
    pub fn row(&self, y: usize) -> &[Rgba] {
        let w = self.size.width as usize;
        &self.pixels[y * w..(y + 1) * w]
    }

    pub fn for_each_row(&mut self, f: impl Fn(usize, &mut [Rgba]) + Sync) {
        super::bands::for_each_row(&mut self.pixels, self.size.width as usize, f);
    }

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

    pub fn map_in_place(&mut self, f: impl Fn(Rgba) -> Rgba + Sync) {
        self.for_each_row(|_, row| {
            for p in row {
                *p = f(*p);
            }
        });
    }

    pub fn map_indexed(&mut self, f: impl Fn(usize, Rgba) -> Rgba + Sync) {
        let w = self.size.width as usize;
        self.for_each_row(|y, row| {
            for (x, p) in row.iter_mut().enumerate() {
                *p = f(y * w + x, *p);
            }
        });
    }

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
        img.pixels_mut()[0] = [1.0, 0.0, 0.0, 1.0];
        img.pixels_mut()[3] = [0.0, 1.0, 0.0, 1.0];
        assert_eq!(img.get_wrapped(0, 0), img.get_wrapped(2, 2));
        assert_eq!(img.get_wrapped(0, 0), img.get_wrapped(-2, -2));
        assert_eq!(img.get_wrapped(1, 1), img.get_wrapped(-1, -1));
    }

    #[test]
    fn bilinear_sample_blends_neighbours_and_wraps() {
        let mut img = Image::new(2);
        img.pixels_mut()[0] = [1.0, 1.0, 1.0, 1.0];
        assert_eq!(
            img.sample_bilinear_wrapped(0.25, 0.25),
            [1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(img.sample_bilinear_wrapped(0.5, 0.25)[0], 0.5);
        assert_eq!(img.sample_bilinear_wrapped(1.0, 0.25)[0], 0.5);
        assert_eq!(img.sample_bilinear_wrapped(0.0, 0.25)[0], 0.5);
    }

    #[test]
    fn bilinear_sample_takes_any_coordinate() {
        let img = Image::fill_uv(4, |u, v| [u, v, 0.0, 1.0]);
        for c in [f32::NAN, f32::INFINITY, -f32::INFINITY, 1e30, -3e38] {
            assert!(img
                .sample_bilinear_wrapped(c, c)
                .iter()
                .all(|x| x.is_finite()));
        }
        assert_eq!(
            img.sample_bilinear_wrapped(-1.875, 3.125),
            img.get_wrapped(0, 0)
        );
    }

    #[test]
    fn fill_uv_samples_pixel_centers() {
        let img = Image::fill_uv(2, |u, _v| [u, 0.0, 0.0, 1.0]);
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
