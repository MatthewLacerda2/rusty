//! SSIL over a lightmap bake (#1750): nothing the bake holds is gathered twice, its
//! specular lobe included. On render's SSIL fixture (`render::test_gpu::ssil`).

mod baked;
mod emission;
mod lobe;
