use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiShader {
    pub name: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use crate::{ImageComponent, TextComponent};

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
