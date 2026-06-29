//! src/shadergen/validate.rs — compose an assembled module through `naga_oil`
//! (#272), the gate that stops a bad shader from shipping.
//!
//! Validation reuses the engine's own load path: it builds a `naga_oil`
//! [`Composer`] pre-loaded with `common.wgsl` (via
//! [`ShaderRegistry::composer_with_common`]) and runs the assembled module string
//! through the same `make_naga_module` the engine calls when it *loads* the baked
//! file (via [`ShaderRegistry::validate_source`]). So "validated at bake" means
//! validated by exactly the code that would later compile it — GPU-free, so it
//! runs in CI with no adapter. A module that references an undefined binding, has
//! a syntax error, or drops a required import fails here and the bake is rejected.

use crate::render::gpu::shaders::ShaderRegistry;

/// Validate an assembled WGSL `module` against `common` rooted at `base`.
///
/// Returns the entry-point names of the composed module on success (so callers
/// can assert contract conformance, e.g. a postfx module exposes `fs_main`), or a
/// message describing why it failed to compose.
pub fn validate(base: &str, module: &str) -> Result<Vec<String>, String> {
    let mut composer = ShaderRegistry::composer_with_common(base)?;
    let naga = ShaderRegistry::validate_source(&mut composer, module)?;
    Ok(naga.entry_points.iter().map(|e| e.name.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "assets/shaders";

    #[test]
    fn a_trivial_postfx_module_composes() {
        // A hand-written minimal postfx module: imports common, has the fullscreen
        // entry points. Proves the validate path reaches a real composed module.
        let module = "#import common::{CameraUniforms}\n\
            @group(0) @binding(1) var t_color: texture_2d<f32>;\n\
            @group(0) @binding(2) var s_color: sampler;\n\
            struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };\n\
            @vertex fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {\n\
                var o: VsOut; o.pos = vec4<f32>(0.0); o.uv = vec2<f32>(0.0); return o;\n }\n\
            @fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32> {\n\
                return textureSample(t_color, s_color, in.uv);\n }";
        let entries = validate(BASE, module).expect("composes");
        assert!(entries.iter().any(|e| e == "fs_main"));
        assert!(entries.iter().any(|e| e == "vs_fullscreen"));
    }

    #[test]
    fn a_syntax_error_fails_validation() {
        let module = "@fragment fn fs_main() -> @location(0) vec4<f32> { this is not wgsl }";
        assert!(validate(BASE, module).is_err());
    }

    #[test]
    fn an_undefined_binding_reference_fails_validation() {
        // References `t_missing`, which is never declared — naga rejects it.
        let module = "@fragment fn fs_main() -> @location(0) vec4<f32> {\n\
            return textureSample(t_missing, s_missing, vec2<f32>(0.0));\n }";
        assert!(validate(BASE, module).is_err());
    }
}
