//! The one `naga_oil` composition path: engine shaders, authored-shader loaders and the
//! bake all compose here, so a bake validates with exactly the code that loads it.

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};

pub use naga::Module;

/// In dependency order: a module is registered after the modules it imports.
pub const MODULES: &[&str] = &[
    "fullscreen.wgsl",
    "lighting/screen_space.wgsl",
    "numeric.wgsl",
    "postfx/ycocg.wgsl",
    "postfx/taa_flicker.wgsl",
    "postfx/taa/io.wgsl",
    "postfx/taa/surface.wgsl",
    "hiz/hidden.wgsl",
    "reactive.wgsl",
    "particles/gpu.wgsl",
    "lighting/sh.wgsl",
    "color/srgb.wgsl",
    "sky/noise.wgsl",
    "sky/look.wgsl",
    "fog/height.wgsl",
    "fog/volumetric.wgsl",
    "lighting/falloff.wgsl",
    "lighting/reflection_probe.wgsl",
    "lighting/ssr_surface.wgsl",
    "lighting/ssr_march.wgsl",
    "lighting/ssr_vogel.wgsl",
    "common.wgsl",
    "lighting/ambient.wgsl",
    "lighting/cascades.wgsl",
    "ibl/ggx.wgsl",
    "sky/motion.wgsl",
    "sky/air.wgsl",
    "forward/bindings.wgsl",
    "forward/maps.wgsl",
    "forward/mapping.wgsl",
    "forward/detail.wgsl",
    "forward/parallax.wgsl",
    "forward/facing.wgsl",
    "forward/specular_aa.wgsl",
    "forward/motion.wgsl",
    "forward/shadow_caster.wgsl",
    "forward/proxy_volume.wgsl",
    "forward/transparent.wgsl",
    "forward/lighting.wgsl",
    "forward/lightmap.wgsl",
    "forward/reflection.wgsl",
    "forward/local_shadow.wgsl",
    "forward/shadowmask.wgsl",
    "postfx/params.wgsl",
    "postfx/frame.wgsl",
    "postfx/grade.wgsl",
    "rt/trace.wgsl",
];

pub fn composer_with_common(base: &str) -> Result<Composer, String> {
    let mut composer = Composer::default();
    for module in MODULES {
        let path = format!("{base}/{module}");
        let source =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        composer
            .add_composable_module(ComposableModuleDescriptor {
                source: &source,
                file_path: module,
                // `common.wgsl` has no `#define_import_path`: without this name
                // `add_composable_module` fails with `NoModuleName`.
                as_name: (*module == "common.wgsl").then(|| "common".to_owned()),
                ..Default::default()
            })
            .map_err(|e| format!("{module} failed to compose: {e:?}"))?;
    }
    Ok(composer)
}

pub fn add_module(composer: &mut Composer, base: &str, module: &str) -> Result<(), String> {
    let path = format!("{base}/{module}");
    let source = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
    add_source(composer, module, &source)
}

pub fn add_source(composer: &mut Composer, module: &str, source: &str) -> Result<(), String> {
    composer
        .add_composable_module(ComposableModuleDescriptor {
            source,
            file_path: module,
            ..Default::default()
        })
        .map(|_| ())
        .map_err(|e| format!("{module} failed to compose: {e:?}"))
}

pub fn compose(composer: &mut Composer, source: &str, file_path: &str) -> Result<Module, String> {
    composer
        .make_naga_module(NagaModuleDescriptor {
            source,
            file_path,
            ..Default::default()
        })
        .map_err(|e| format!("{e:?}"))
}
