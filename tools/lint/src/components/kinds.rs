//! The Add Component menu names its entries by `ComponentKind` (#939: one table
//! over `authoring::has_kind`), so the `.has_<field>(id)` guard axis 2 looks for
//! lives in `has_kind`'s match arms. This reads those arms as the kind → field map.

/// The file holding `authoring::has_kind`, whose arms map a kind to its guard.
pub(super) const HAS_KIND: &str = "src/scene/authoring/dependency/dispatch.rs";

/// The Add menu source plus, for every `ComponentKind::<Kind>` it names, the
/// `.has_<field>(id)` guard `has_kind` resolves that kind to — so an entry in the
/// table counts exactly as a hand-written `!world.has_<field>(id)` gate does.
pub(super) fn with_kind_guards(add_src: &str, has_kind_src: &str) -> String {
    let mut out = add_src.to_string();
    for (kind, field) in kind_fields(has_kind_src) {
        if names_kind(add_src, &kind) {
            out.push_str(&format!("\n.has_{field}(id)"));
        }
    }
    out
}

/// `(Kind, field)` for every `ComponentKind::<Kind> => world.has_<field>(id)` arm.
fn kind_fields(src: &str) -> Vec<(String, String)> {
    src.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("ComponentKind::")?;
            let (kind, guard) = rest.split_once(" => ")?;
            let field = guard.split_once(".has_")?.1.strip_suffix("(id),")?;
            Some((kind.to_string(), field.to_string()))
        })
        .collect()
}

/// Whether `src` names `ComponentKind::<kind>` as a whole path (`Mask` is not
/// named by `ComponentKind::MaskFoo`).
fn names_kind(src: &str, kind: &str) -> bool {
    let needle = format!("ComponentKind::{kind}");
    src.match_indices(&needle).any(|(at, _)| {
        src[at + needle.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAS_KIND_SRC: &str = "    match kind {
        ComponentKind::Texture => world.has_material(id),
        ComponentKind::Mask => world.has_mask(id),
    }";

    #[test]
    fn a_table_entry_counts_as_its_fields_guard() {
        let add = "(ComponentKind::Texture, \"\", \"Material\"),";
        assert!(with_kind_guards(add, HAS_KIND_SRC).contains(".has_material(id)"));
    }

    #[test]
    fn a_kind_the_menu_never_names_gets_no_guard() {
        let add = "(ComponentKind::MaskLayer, \"\", \"Other\"),";
        let out = with_kind_guards(add, HAS_KIND_SRC);
        assert!(!out.contains(".has_mask(id)"));
        assert!(!out.contains(".has_material(id)"));
    }
}
