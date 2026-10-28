//! src/procgen/bands.rs — split an image's rows into contiguous bands, one thread
//! each (#1490).
//!
//! Every op writes each output pixel as a pure function of its inputs and its
//! position, so a band writes its own slice of the output and nothing else: no
//! shared accumulator, no order dependence. The result is byte-identical whatever
//! the band count, which is why the count may follow the machine's cores
//! ([`std::thread::available_parallelism`]) inside a sim module. [`with_bands`] pins
//! the count so a test can prove that on one recipe.

use std::cell::Cell;
use std::thread;

use super::image_buf::Rgba;

/// Images with fewer pixels than this (128²) run serially: thread start-up would
/// cost more than the work, and the tests and thumbnails are all this small.
const SERIAL_BELOW: usize = 128 * 128;

thread_local! {
    /// The band count [`with_bands`] pinned on this thread, if any.
    static PINNED: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Run `f` with every op on this thread split into exactly `bands` bands (at
/// least 1, at most one per row), whatever the image size or core count. A
/// determinism probe: the output must not change with it.
pub fn with_bands<R>(bands: usize, f: impl FnOnce() -> R) -> R {
    /// Restores the previous pin even if `f` panics.
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PINNED.with(|p| p.set(self.0));
        }
    }
    let _restore = Restore(PINNED.with(|p| p.replace(Some(bands))));
    f()
}

/// How many bands `rows` rows of `pixels` pixels split into.
fn band_count(rows: usize, pixels: usize) -> usize {
    let n = PINNED.with(Cell::get).unwrap_or_else(|| {
        if pixels < SERIAL_BELOW {
            1
        } else {
            thread::available_parallelism().map_or(1, |n| n.get())
        }
    });
    n.clamp(1, rows.max(1))
}

/// Call `f(y, row)` for every `width`-pixel row of `pixels`, the rows split into
/// contiguous bands run in parallel. `f` sees each row exactly once.
pub fn for_each_row(pixels: &mut [Rgba], width: usize, f: impl Fn(usize, &mut [Rgba]) + Sync) {
    if width == 0 {
        return;
    }
    let rows = pixels.len() / width;
    let bands = band_count(rows, pixels.len());
    let per = rows.div_ceil(bands).max(1);
    let run = |first: usize, band: &mut [Rgba]| {
        for (i, row) in band.chunks_mut(width).enumerate() {
            f(first + i, row);
        }
    };
    if bands == 1 {
        return run(0, pixels);
    }
    thread::scope(|s| {
        for (b, band) in pixels.chunks_mut(per * width).enumerate() {
            let run = &run;
            s.spawn(move || run(b * per, band));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each row tagged with its index, under `bands` bands.
    fn tagged(rows: usize, bands: usize) -> Vec<Rgba> {
        let mut px = vec![[0.0; 4]; rows * 3];
        with_bands(bands, || {
            for_each_row(&mut px, 3, |y, row| row.fill([y as f32, 0.0, 0.0, 1.0]))
        });
        px
    }

    #[test]
    fn every_row_is_visited_once_with_its_own_index() {
        let serial = tagged(10, 1);
        for (i, p) in serial.iter().enumerate() {
            assert_eq!(p[0], (i / 3) as f32);
        }
        // Uneven splits, and more bands than rows.
        for bands in [2, 3, 4, 7, 10, 64] {
            assert_eq!(tagged(10, bands), serial, "{bands} bands");
        }
    }

    #[test]
    fn the_pin_is_restored_after_the_call() {
        with_bands(5, || assert_eq!(band_count(100, 1), 5));
        assert_eq!(band_count(100, 1), 1, "small images run serially again");
        assert_eq!(with_bands(0, || band_count(100, 1)), 1, "zero reads as one");
        assert_eq!(with_bands(9, || band_count(4, 1)), 4, "at most one per row");
    }
}
