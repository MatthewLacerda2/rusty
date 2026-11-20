//! Which material slots sample each image of a written `.glb` (#2365): what a
//! texture a conversion extracted is compressed by, since it has no sidecar of its
//! own (`texture_ref::extracted`). Read from the JSON chunk alone, as
//! [`super::image_uris`] is.

use super::read::json_chunk;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// What a glTF slot reads from its image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    /// sRGB colour: `baseColorTexture`, `emissiveTexture`.
    Colour,
    /// A tangent-space normal map: `normalTexture`.
    Normal,
    /// Its red channel alone, raw: `occlusionTexture`.
    Red,
    /// Several raw channels: `metallicRoughnessTexture` (G and B),
    /// `KHR_materials_specular`'s `specularTexture` (A).
    Data,
}

/// The slots every material of the `.glb` at `path` samples each image in, by the
/// image's URI as written; an image no material samples is absent. `None` when the
/// file is not a `.glb`.
pub fn image_slots(path: &Path) -> Option<BTreeMap<String, Vec<Slot>>> {
    let doc: Value = serde_json::from_slice(&json_chunk(path)?).ok()?;
    let list = |key: &str| doc.get(key).and_then(Value::as_array);
    let uri_of = |texture: &Value| {
        let index = texture.get("index")?.as_u64()?;
        let source = list("textures")?.get(usize::try_from(index).ok()?)?;
        let image = list("images")?.get(usize::try_from(source.get("source")?.as_u64()?).ok()?)?;
        image.get("uri")?.as_str().map(str::to_string)
    };
    let mut slots: BTreeMap<String, Vec<Slot>> = BTreeMap::new();
    for material in list("materials").into_iter().flatten() {
        for (texture, slot) in textures(material) {
            if let Some(uri) = uri_of(texture) {
                let of = slots.entry(uri).or_default();
                if !of.contains(&slot) {
                    of.push(slot);
                }
            }
        }
    }
    Some(slots)
}

/// Each texture reference `material` holds, with the slot it fills.
fn textures(material: &Value) -> Vec<(&Value, Slot)> {
    let at = |path: &[&str]| path.iter().try_fold(material, |v, key| v.get(key));
    let named = [
        (
            &["pbrMetallicRoughness", "baseColorTexture"][..],
            Slot::Colour,
        ),
        (
            &["pbrMetallicRoughness", "metallicRoughnessTexture"],
            Slot::Data,
        ),
        (&["normalTexture"], Slot::Normal),
        (&["occlusionTexture"], Slot::Red),
        (&["emissiveTexture"], Slot::Colour),
        (
            &["extensions", "KHR_materials_specular", "specularTexture"],
            Slot::Data,
        ),
        (
            &[
                "extensions",
                "KHR_materials_specular",
                "specularColorTexture",
            ],
            Slot::Colour,
        ),
    ];
    named
        .into_iter()
        .filter_map(|(path, slot)| Some((at(path)?, slot)))
        .collect()
}
