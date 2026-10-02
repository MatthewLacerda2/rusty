//! src/shadergen/compose.rs — the one GPU-free `naga_oil` composition path (#722).
//!
//! Every WGSL module the engine compiles goes through here: `ShaderRegistry::load`
//! for the engine's own shaders, the authored post-FX / UI / preview loaders, and
//! [`super::validate`] at bake time. Keeping it a single path is what makes
//! "validated at bake" mean "validated by exactly the code that would later compile
//! it". It lives in `shadergen`, not `render`, so the sim side (scene authoring, the
//! Lua `Shader.Bake`) can validate a shader without compiling the renderer: `render`
//! depends on this module, never the other way.
//!
//! **Version lockstep:** `naga_oil 0.13` composes into `naga 0.19`, the same
//! version wgpu 0.19.x links against. Upgrade all three as a unit.

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};

/// A composed, validated shader module — the IR wgpu takes as `ShaderSource::Naga`.
pub use naga::Module;

/// Build a composer pre-loaded with `<base>/common.wgsl` as the `"common"` module,
/// the context every engine shader is compiled in.
///
/// Returns a message if `common.wgsl` is missing or fails to compose.
pub fn composer_with_common(base: &str) -> Result<Composer, String> {
    let mut composer = Composer::default();
    let common_path = format!("{base}/common.wgsl");
    let common_src = std::fs::read_to_string(&common_path)
        .map_err(|e| format!("cannot read {common_path}: {e}"))?;
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
        .map_err(|e| format!("common.wgsl failed to compose: {e:?}"))?;
    Ok(composer)
}

/// Compose a WGSL `source` against the composer's `common` module into a validated
/// naga module. `file_path` only labels errors. Errors come back as a message, so
/// each caller picks its own policy: the engine's startup set panics, authored
/// shaders skip with a warning, the bake rejects.
pub fn compose(composer: &mut Composer, source: &str, file_path: &str) -> Result<Module, String> {
    composer
        .make_naga_module(NagaModuleDescriptor {
            source,
            file_path,
            ..Default::default()
        })
        .map_err(|e| format!("{e:?}"))
}
