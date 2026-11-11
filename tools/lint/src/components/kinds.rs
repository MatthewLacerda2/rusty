//! The Add Component menu is generated from the registry (#2023): it expands
//! `first_class!` into one entry per row, each guarded by `authoring::has_kind`, so
//! the `.has_<field>(id)` guard axis 2 looks for is there for every row. This reads
//! the registry's rows as those guards.

use super::discover::rows;

/// The Add menu source plus, when it expands the registry (`first_class!(`), the
/// `.has_<field>(id)` guard of every registry row in `registry_src` — so a generated
/// entry counts exactly as a hand-written `!world.has_<field>(id)` gate does.
pub(super) fn with_kind_guards(add_src: &str, registry_src: &str) -> String {
    let mut out = add_src.to_string();
    if add_src.contains("first_class!(") {
        for (field, _) in rows(registry_src) {
            out.push_str(&format!("\n.has_{field}(id)"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY_SRC: &str = "macro_rules! first_class {
            material: material::MaterialComponent => Texture [material_mut has_material];
            mask: ui::mask::MaskComponent => Mask [mask_mut has_mask];
    }";

    #[test]
    fn a_menu_expanding_the_registry_guards_every_row() {
        let add = "first_class!(entries);";
        let out = with_kind_guards(add, REGISTRY_SRC);
        assert!(out.contains(".has_material(id)") && out.contains(".has_mask(id)"));
    }

    #[test]
    fn a_menu_that_does_not_expand_it_gets_no_guard() {
        let add = "(ComponentKind::Mask, \"\", \"Other\"),";
        let out = with_kind_guards(add, REGISTRY_SRC);
        assert!(!out.contains(".has_mask(id)"));
        assert!(!out.contains(".has_material(id)"));
    }
}
