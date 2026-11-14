//! Bakes as the renderer draws them: GPU tests that bake lightmaps (which only
//! `dev` does) and check the frame. They sit here, above `render`, and reach it
//! through its public API and its test hooks (`render::test_gpu`).

mod shadowmask;
mod ssil;
