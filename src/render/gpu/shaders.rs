//! src/render/shaders.rs — runtime WGSL loader backed by naga_oil.
//!
//! [`ShaderRegistry`] reads `.wgsl` files from disk at runtime (not via
//! `include_str!`) and pre-processes them through a `naga_oil` [`Composer`]
//! before handing the result to wgpu as a compiled [`naga`] module. This lets
//! shaders be edited without recompiling the Rust crate, and allows passes to
//! share type definitions via `#import "common"` rather than duplicating them.
//!
//! The composition itself — `common.wgsl` registration and `make_naga_module` —
//! is [`crate::shadergen::compose`], the GPU-free path the shader bake validates
//! through too (#722). This file keeps only the disk lookup and the wgpu handoff.

use naga_oil::compose::Composer;

use crate::shadergen::compose;

/// Resolves and compiles WGSL shaders at runtime using a naga_oil Composer.
///
/// The registry is pre-loaded with `common.wgsl` from the shader base
/// directory so any shader that writes `#import "common"` automatically gets
/// the shared [`CameraUniforms`] and [`VertexInput`] struct definitions.
pub struct ShaderRegistry {
    composer: Composer,
    base: String,
}

impl ShaderRegistry {
    /// Build a registry rooted at `base` (e.g. `"assets/shaders"`).
    /// Reads and registers `<base>/common.wgsl` as the `"common"` module.
    ///
    /// # Panics
    /// Panics if `common.wgsl` cannot be read or fails naga_oil validation —
    /// the shader files are part of the shipped binary's asset set and a
    /// missing or invalid common module is a packaging error, not a runtime
    /// condition.
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into();
        let composer =
            compose::composer_with_common(&base).unwrap_or_else(|e| panic!("ShaderRegistry: {e}"));
        Self { composer, base }
    }

    /// Resolve `<base>/<name>` through [`compose::compose`] (handling any
    /// `#import "common"` directives) into a validated naga module. GPU-free:
    /// the wgpu device handoff lives in [`load`], so this path is unit-testable
    /// without an adapter.
    ///
    /// # Panics
    /// Panics on missing files or shader errors — both are authoring mistakes
    /// caught during engine startup, not recoverable runtime conditions.
    fn compose(&mut self, name: &str) -> compose::Module {
        let path = format!("{}/{name}", self.base);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("ShaderRegistry: cannot read {path}: {e}"));
        compose::compose(&mut self.composer, &source, &path)
            .unwrap_or_else(|e| panic!("ShaderRegistry: {path} failed to compose: {e}"))
    }

    /// Load `<base>/<name>`, run it through the naga_oil Composer (resolving
    /// any `#import "common"` directives), and create a wgpu shader module.
    ///
    /// # Panics
    /// Panics on missing files or shader errors — both are authoring mistakes
    /// caught during engine startup, not recoverable runtime conditions.
    pub fn load(&mut self, device: &wgpu::Device, name: &str, label: &str) -> wgpu::ShaderModule {
        let naga_module = self.compose(name);
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(naga_module)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ShaderRegistry;
    use crate::shadergen::validate::validate;

    const SHIPPED: [&str; 7] = [
        "shader.wgsl",
        "skybox.wgsl",
        "sky_gradient.wgsl",
        "particles.wgsl",
        "ribbons.wgsl",
        "postfx.wgsl",
        "ui.wgsl",
    ];

    /// Every shipped shader must compose against the registered `common` module
    /// without a GPU. This catches `#import` / module-name regressions on Linux
    /// CI — the GPU screenshot test only exercises this path on macOS/Windows.
    #[test]
    fn all_shaders_compose() {
        let mut registry = ShaderRegistry::new("assets/shaders");
        for name in SHIPPED {
            let module = registry.compose(name);
            assert!(
                !module.entry_points.is_empty(),
                "{name} composed but has no entry points"
            );
        }
    }

    /// "Validated at bake" means validated by the code that loads it (#722): the
    /// bake's `validate` and the registry's load agree on every shipped shader's
    /// composed entry points, and a module one rejects the other rejects too.
    #[test]
    fn bake_validation_and_load_compose_identically() {
        let mut registry = ShaderRegistry::new("assets/shaders");
        for name in SHIPPED {
            let source = std::fs::read_to_string(format!("assets/shaders/{name}")).unwrap();
            let baked = validate("assets/shaders", &source).expect("bake validates");
            let loaded: Vec<String> = registry
                .compose(name)
                .entry_points
                .iter()
                .map(|e| e.name.clone())
                .collect();
            assert_eq!(baked, loaded, "{name}: bake and load disagree");
        }

        let dir = std::env::temp_dir().join(format!("rusty-722-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy("assets/shaders/common.wgsl", dir.join("common.wgsl")).unwrap();
        // Names a `common` struct that does not exist: naga rejects the unknown type.
        let bad = "#import common::{CameraUniforms}\n\
            @group(0) @binding(0) var<uniform> camera: NoSuchStruct;\n\
            @fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.0); }";
        std::fs::write(dir.join("bad.wgsl"), bad).unwrap();
        let base = dir.to_string_lossy().into_owned();
        assert!(
            validate(&base, bad).is_err(),
            "bake accepted a broken module"
        );
        let load =
            std::panic::catch_unwind(|| ShaderRegistry::new(base.clone()).compose("bad.wgsl"));
        assert!(load.is_err(), "load accepted a module the bake rejects");
        std::fs::remove_dir_all(&dir).ok();
    }
}
