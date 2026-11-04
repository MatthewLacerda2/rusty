//! TAAU (#1702, part 3): TAA's resolve upscaling a scaled view to its output, in
//! place of the bilinear upscale (`Renderer::upscale_scene`).
//!
//! **Where it runs: at TAA's place, Unity's order.** Depth of field runs at the
//! scene's size, as in URP; the resolve then reconstructs the output-size image
//! from it (`TaaPasses::upscale`, in `PostFx::run_taa`), and motion blur, the
//! exposure meter, bloom, the composite, the authored effects and the UI's
//! backdrop read that resolved image at the output's size
//! (`PostFx::chain_size`), as URP's STP and HDRP's FSR2 and DLSS place it. So
//! bloom, film grain, a screen-space pattern and a HUD glass panel see a stable
//! image, and the chain writes the output: nothing upscales twice
//! (`PostFx::taa_upscaled`). What the scale saves is the scene's shading; the
//! chain after TAA costs what it costs at the output's size.
//!
//! **The reconstruction** (`fs_upscale` in `postfx/taa.wgsl`): each output pixel
//! gathers the 3×3 scene pixels around it, each weighted by its jittered sample's
//! distance to the output pixel's centre (URP's `CalculateFilterWeights`, a
//! Gaussian fit of Blackman-Harris), blends that into the history as the scene
//! resolve does, with the frame's weight scaled by how near the nearest sample
//! landed, so a sample far from the pixel only nudges it.

#[cfg(test)]
mod ghost_gpu_tests;
#[cfg(test)]
mod gpu_tests;
