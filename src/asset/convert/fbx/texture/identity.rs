//! The identity an extracted texture keeps across re-conversions (#1130).
//!
//! A saved scene names an embedded texture by its source model and this identity
//! (`asset::convert::texture_ref`), never by its file in the cache: that file's
//! name takes a `.1` suffix when another texture of the same file name comes
//! first, so an edit to the FBX can move it. The identity is the texture
//! element's own name, as Unity keys an embedded texture. When several elements
//! share a name the first keeps it, so a reference saved before a namesake was
//! added still resolves; each later one, and one the FBX gives no name, is
//! `#<index>` among the scene's textures (no element name is read as one: a name
//! starting with `#` takes the index form too).
//!
//! **The accepted risk:** the `#<index>` form is only as stable as the order of
//! the textures. A texture inserted before one so named shifts its index, and a
//! saved reference then names its neighbour, or nothing.

use crate::core::collections::Map;

/// Each texture's identity, by FBX element id.
pub fn identities<'a>(textures: impl IntoIterator<Item = &'a ufbx::Texture>) -> Map<u32, String> {
    let mut named: Vec<String> = Vec::new();
    textures
        .into_iter()
        .enumerate()
        .map(|(index, t)| {
            let own = t.element.name.to_string();
            let usable = !own.is_empty() && !own.starts_with('#') && !named.contains(&own);
            let identity = if usable {
                named.push(own.clone());
                own
            } else {
                format!("#{index}")
            };
            (t.element.element_id, identity)
        })
        .collect()
}
