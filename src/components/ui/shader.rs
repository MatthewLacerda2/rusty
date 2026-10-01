//! src/components/ui/shader.rs — the custom shader a UI graphic draws with (#427).
//!
//! Unity's `Graphic.material`. An `Image`, `Shape` or `Text` naming a [`UiShader`] draws
//! through that baked ui shader (`Shader.Bake` with `pass = "ui"`, written as
//! `<name>.wgsl`) instead of the standard UI shader: glitch, scanlines, hologram,
//! dissolve, wipes. `params` holds the shader's runtime params by canonical name
//! (`"dissolve.amount"`, #399's naming), set from scripts with `UI.SetShaderParam`;
//! a param not set draws with its baked default. Pure authoring data — the renderer
//! resolves the name, and falls back to the standard shader if it cannot.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A graphic's custom ui shader and its runtime param values. See the module docs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiShader {
    /// The baked ui shader's name: `<name>.wgsl` in the authored shader workspace
    /// (else the engine set).
    pub name: String,
    /// Runtime param values by canonical name; absent ones draw with the default.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use crate::components::{ImageComponent, TextComponent};

    use super::*;

    #[test]
    fn a_graphic_shader_round_trips_and_is_omitted_when_none() {
        let mut image = ImageComponent::default();
        let plain = serde_json::to_string(&image).unwrap();
        assert!(!plain.contains("shader"), "{plain}");
        image.shader = Some(UiShader {
            name: "glitch".into(),
            params: [("glitch_slices.amount".to_string(), vec![4.0])].into(),
        });
        let json = serde_json::to_string(&image).unwrap();
        assert_eq!(
            serde_json::from_str::<ImageComponent>(&json).unwrap(),
            image
        );
        let text: TextComponent = serde_json::from_str(r#"{"shader":{"name":"holo"}}"#).unwrap();
        assert_eq!(text.shader.unwrap().name, "holo");
    }
}
