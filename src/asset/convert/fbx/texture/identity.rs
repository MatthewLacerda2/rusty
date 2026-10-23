//! The identity an extracted texture keeps across re-conversions (#1130).
//!
//! A saved scene names an embedded texture by its source model and this identity
//! (`asset::convert::texture_ref`), never by its file in the cache: that file's
//! name takes a `.1` suffix when another texture of the same file name comes
//! first, so an edit to the FBX can move it. The identity is the texture
//! element's own name, as Unity keys an embedded texture; when the FBX gives
//! none, or two textures share one, it is `#<index>` among the scene's textures
//! (no element name is read as one: a name starting with `#` takes the index
//! form too).

use crate::core::collections::Map;

/// Each texture's identity, by FBX element id.
pub fn identities<'a>(textures: impl IntoIterator<Item = &'a ufbx::Texture>) -> Map<u32, String> {
    let textures: Vec<&ufbx::Texture> = textures.into_iter().collect();
    let name = |t: &ufbx::Texture| t.element.name.to_string();
    let shared = |n: &str| textures.iter().filter(|t| name(t) == n).count() > 1;
    textures
        .iter()
        .enumerate()
        .map(|(index, t)| {
            let own = name(t);
            let usable = !own.is_empty() && !own.starts_with('#') && !shared(&own);
            let identity = if usable { own } else { format!("#{index}") };
            (t.element.element_id, identity)
        })
        .collect()
}
