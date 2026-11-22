//! A saved scene names an embedded texture by this identity, never by its cache
//! file (a `.1` suffix can move it). It is the element's own name, as Unity keys
//! one; a later namesake or an unnamed element is `#<index>`, which is only as
//! stable as the textures' order.

use rusty_core::collections::Map;

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
