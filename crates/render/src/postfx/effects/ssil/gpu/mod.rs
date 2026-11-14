//! SSIL on a GPU (#1750): a point light on a red wall tints the white floor, an
//! indirect-only scene gains nothing, off is off, a cut or a light switched off
//! leaves nothing behind, a turning camera doesn't crawl, and a NaN never spreads.
//! Over a lightmap bake, which only `dev` makes, nothing is gathered twice: those
//! tests are `dev`'s (`dev::bake::drawn::ssil`), on the same fixture
//! (`test_gpu::ssil`).

mod bounce;
mod history;
mod occlusion;
mod turn;
