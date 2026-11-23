use serde::de::value::MapDeserializer;
use serde::Deserialize;

use super::MaterialAsset;

pub(super) fn default_base_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

pub(super) fn default_roughness() -> f32 {
    0.5
}

pub(super) fn default_alpha() -> f32 {
    1.0
}

pub(super) fn default_alpha_cutoff() -> f32 {
    0.5
}

pub(super) fn default_true() -> bool {
    true
}

/// The empty document: every field takes its `#[serde(default)]`, so each default is said once,
/// where a scene file that omits the key reads it.
impl Default for MaterialAsset {
    fn default() -> Self {
        let empty = std::iter::empty::<(&str, ())>();
        let doc = MapDeserializer::<_, serde::de::value::Error>::new(empty);
        Self::deserialize(doc).expect("every field of MaterialAsset has a serde default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_empty_document_and_round_trips() {
        let m = MaterialAsset::default();
        assert_eq!(
            (m.base_color, m.roughness, m.alpha_cutoff),
            ([1.0; 3], 0.5, 0.5)
        );
        assert_eq!((m.height_min_steps, m.height_max_steps), (5, 15));
        assert!(m.receive_shadows && m.receive_decals && m.preserve_specular);
        let back: MaterialAsset =
            serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
    }
}
