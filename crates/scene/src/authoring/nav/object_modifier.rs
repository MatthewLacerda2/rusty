//! crates/scene/src/authoring/nav/object_modifier.rs — Shared NavMeshModifier authoring ops (#1959).
//!
//! The ONE place the engine mutates an entity's `NavMeshModifierComponent`. The
//! editor's card and `Component.Set` both route every write through these, so the
//! one rule lives once: an override area is a valid id (`0..MAX_AREAS`).
//!
//! Pure.

use crate::nav_settings::MAX_AREAS;
use rusty_components::NavMeshModifierComponent as Modifier;

/// Enable or disable the modifier.
pub fn set_active(m: &mut Modifier, active: bool) {
    m.active = active;
}

/// Set the area this object bakes as, `None` to keep the bake's. An id past the
/// area table's limit is ignored.
pub fn set_override_area(m: &mut Modifier, area: Option<i64>) {
    match area {
        None => m.override_area = None,
        Some(a) if (0..MAX_AREAS as i64).contains(&a) => m.override_area = Some(a as u8),
        Some(_) => {}
    }
}

/// Leave this object's colliders out of the bake, or put them back.
pub fn set_ignore_from_build(m: &mut Modifier, ignore: bool) {
    m.ignore_from_build = ignore;
}

/// Whether the modifier covers the entity's descendants.
pub fn set_apply_to_children(m: &mut Modifier, apply: bool) {
    m.apply_to_children = apply;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_out_of_range_area_is_ignored_and_none_clears() {
        let mut m = Modifier::default();
        set_override_area(&mut m, Some(1));
        set_override_area(&mut m, Some(32));
        set_override_area(&mut m, Some(-1));
        assert_eq!(m.override_area, Some(1));
        set_override_area(&mut m, None);
        assert_eq!(m.override_area, None);
        set_ignore_from_build(&mut m, true);
        set_apply_to_children(&mut m, false);
        set_active(&mut m, false);
        assert!(m.ignore_from_build && !m.apply_to_children && !m.active);
    }
}
