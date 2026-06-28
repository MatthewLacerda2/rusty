//! src/render/shaders.rs — runtime WGSL loader backed by naga_oil.
//!
//! [`ShaderRegistry`] reads `.wgsl` files from disk at runtime (not via
//! `include_str!`) and pre-processes them through a `naga_oil` [`Composer`]
//! before handing the result to wgpu as a compiled [`naga`] module. This lets
//! shaders be edited without recompiling the Rust crate, and allows passes to
//! share type definitions via `#import "common"` rather than duplicating them.
//!
//! **Version lockstep:** `naga_oil 0.13` requires `naga 0.19`, which is the
//! same version wgpu 0.19.x links against. Upgrade all three as a unit; see
//! `Cargo.toml` for the pinned version comment.

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};

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
        let mut composer = Composer::default();
        let common_path = format!("{base}/common.wgsl");
        let common_src = std::fs::read_to_string(&common_path)
            .unwrap_or_else(|e| panic!("ShaderRegistry: cannot read {common_path}: {e}"));
        composer
            .add_composable_module(ComposableModuleDescriptor {
                source: &common_src,
                file_path: "common",
                // The module's import name. naga_oil derives it from a
                // `#define_import_path` directive in the source or this field;
                // `common.wgsl` has neither directive, so name it here or
                // `add_composable_module` fails with `NoModuleName` and every
                // `#import "common"` is unresolvable.
                as_name: Some("common".to_owned()),
                ..Default::default()
            })
            .unwrap_or_else(|e| panic!("ShaderRegistry: common.wgsl failed to compose: {e:?}"));
        Self { composer, base }
    }

    /// Resolve `<base>/<name>` through the Composer (handling any
    /// `#import "common"` directives) into a validated naga module. GPU-free:
    /// the wgpu device handoff lives in [`load`], so this path is unit-testable
    /// without an adapter.
    ///
    /// # Panics
    /// Panics on missing files or shader errors — both are authoring mistakes
    /// caught during engine startup, not recoverable runtime conditions.
    fn compose(&mut self, name: &str) -> wgpu::naga::Module {
        let path = format!("{}/{name}", self.base);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("ShaderRegistry: cannot read {path}: {e}"));
        self.composer
            .make_naga_module(NagaModuleDescriptor {
                source: &source,
                file_path: &path,
                ..Default::default()
            })
            .unwrap_or_else(|e| panic!("ShaderRegistry: {path} failed to compose: {e:?}"))
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

    /// Every shipped shader must compose against the registered `common` module
    /// without a GPU. This catches `#import` / module-name regressions on Linux
    /// CI — the GPU screenshot test only exercises this path on macOS/Windows.
    #[test]
    fn all_shaders_compose() {
        let mut registry = ShaderRegistry::new("assets/shaders");
        for name in [
            "shader.wgsl",
            "shadow.wgsl",
            "skybox.wgsl",
            "sky_gradient.wgsl",
            "particles.wgsl",
            "decals.wgsl",
            "postfx.wgsl",
        ] {
            let module = registry.compose(name);
            assert!(
                !module.entry_points.is_empty(),
                "{name} composed but has no entry points"
            );
        }
    }
}
