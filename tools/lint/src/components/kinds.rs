use super::discover::rows;
use crate::code::code_lines;

pub(super) fn with_kind_guards(add_src: &str, registry_src: &str) -> String {
    let mut out = add_src.to_string();
    if code_lines(add_src)
        .iter()
        .any(|l| l.contains("first_class!("))
    {
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
    fn a_menu_that_only_mentions_the_registry_in_a_comment_gets_no_guard() {
        let add = "// first_class!(entries);\n/* first_class!(entries) */ fn menu() {}";
        let out = with_kind_guards(add, REGISTRY_SRC);
        assert!(!out.contains(".has_mask(id)") && !out.contains(".has_material(id)"));
    }

    #[test]
    fn a_menu_that_does_not_expand_it_gets_no_guard() {
        let add = "(ComponentKind::Mask, \"\", \"Other\"),";
        let out = with_kind_guards(add, REGISTRY_SRC);
        assert!(!out.contains(".has_mask(id)"));
        assert!(!out.contains(".has_material(id)"));
    }
}
