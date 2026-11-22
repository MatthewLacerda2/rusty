use naga_oil::compose::Composer;

use rusty_shadergen::compose;

pub struct ShaderRegistry {
    composer: Composer,
    base: String,
    added: Vec<String>,
}

impl ShaderRegistry {
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into();
        let composer =
            compose::composer_with_common(&base).unwrap_or_else(|e| panic!("ShaderRegistry: {e}"));
        Self {
            composer,
            base,
            added: Vec::new(),
        }
    }

    pub fn add_module(&mut self, module: &str) {
        if self.added.iter().any(|m| m == module) {
            return;
        }
        compose::add_module(&mut self.composer, &self.base, module)
            .unwrap_or_else(|e| panic!("ShaderRegistry: {e}"));
        self.added.push(module.to_string());
    }

    /// Compose `source` (labelled `name` in errors) into a validated naga module.
    ///
    /// # Panics
    /// On a shader error, as [`load`](Self::load) does.
    pub fn compose_source(&mut self, source: &str, name: &str) -> compose::Module {
        compose::compose(&mut self.composer, source, name)
            .unwrap_or_else(|e| panic!("ShaderRegistry: {name} failed to compose: {e}"))
    }

    pub fn load_source(
        &mut self,
        device: &wgpu::Device,
        source: &str,
        name: &str,
    ) -> wgpu::ShaderModule {
        let naga_module = self.compose_source(source, name);
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(naga_module)),
        })
    }

    fn compose(&mut self, name: &str) -> compose::Module {
        let path = format!("{}/{name}", self.base);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("ShaderRegistry: cannot read {path}: {e}"));
        compose::compose(&mut self.composer, &source, &path)
            .unwrap_or_else(|e| panic!("ShaderRegistry: {path} failed to compose: {e}"))
    }

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
    use rusty_shadergen::compose;
    use rusty_shadergen::validate::validate;

    const SHIPPED: [&str; 16] = [
        "shader.wgsl",
        "outline/mask.wgsl",
        "outline/edge.wgsl",
        "skybox.wgsl",
        "sky_gradient.wgsl",
        "particles.wgsl",
        "ribbons.wgsl",
        "postfx/bloom.wgsl",
        "postfx/copy.wgsl",
        "postfx/fxaa.wgsl",
        "postfx/exposure.wgsl",
        "postfx/motion_blur.wgsl",
        "postfx/depth_of_field.wgsl",
        "postfx/taa.wgsl",
        "ui.wgsl",
        "gizmos/navmesh.wgsl",
    ];

    /// Every shipped shader must compose against the registered `common` module
    /// without a GPU, so an `#import` / module-name regression fails here before
    /// any adapter is asked for.
    #[test]
    fn all_shaders_compose() {
        let mut registry = ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
        for name in SHIPPED {
            let module = registry.compose(name);
            assert!(
                !module.entry_points.is_empty(),
                "{name} composed but has no entry points"
            );
        }
    }

    #[test]
    fn bake_validation_and_load_compose_identically() {
        let mut registry = ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
        for name in SHIPPED {
            let source =
                std::fs::read_to_string(format!("{}/{name}", rusty_shadergen::engine_shader_dir()))
                    .unwrap();
            let baked =
                validate(rusty_shadergen::engine_shader_dir(), &source).expect("bake validates");
            let loaded: Vec<String> = registry
                .compose(name)
                .entry_points
                .iter()
                .map(|e| e.name.clone())
                .collect();
            assert_eq!(baked, loaded, "{name}: bake and load disagree");
        }

        let dir = rusty_test_support::temp::dir().join("rusty-722");
        std::fs::create_dir_all(&dir).unwrap();
        for module in compose::MODULES {
            let to = dir.join(module);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(
                format!("{}/{module}", rusty_shadergen::engine_shader_dir()),
                to,
            )
            .unwrap();
        }
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
