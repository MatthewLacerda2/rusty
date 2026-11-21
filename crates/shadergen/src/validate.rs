use super::compose;

pub fn validate(base: &str, module: &str) -> Result<Vec<String>, String> {
    let mut composer = compose::composer_with_common(base)?;
    let naga = compose::compose(&mut composer, module, "authored.wgsl")?;
    Ok(naga.entry_points.iter().map(|e| e.name.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trivial_postfx_module_composes() {
        let module = "#import common::{CameraUniforms}\n\
            @group(0) @binding(1) var t_color: texture_2d<f32>;\n\
            @group(0) @binding(2) var s_color: sampler;\n\
            struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };\n\
            @vertex fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {\n\
                var o: VsOut; o.pos = vec4<f32>(0.0); o.uv = vec2<f32>(0.0); return o;\n }\n\
            @fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32> {\n\
                return textureSample(t_color, s_color, in.uv);\n }";
        let entries = validate(crate::engine_shader_dir(), module).expect("composes");
        assert!(entries.iter().any(|e| e == "fs_main"));
        assert!(entries.iter().any(|e| e == "vs_fullscreen"));
    }

    #[test]
    fn a_syntax_error_fails_validation() {
        let module = "@fragment fn fs_main() -> @location(0) vec4<f32> { this is not wgsl }";
        assert!(validate(crate::engine_shader_dir(), module).is_err());
    }

    #[test]
    fn an_undefined_binding_reference_fails_validation() {
        let module = "@fragment fn fs_main() -> @location(0) vec4<f32> {\n\
            return textureSample(t_missing, s_missing, vec2<f32>(0.0));\n }";
        assert!(validate(crate::engine_shader_dir(), module).is_err());
    }
}
