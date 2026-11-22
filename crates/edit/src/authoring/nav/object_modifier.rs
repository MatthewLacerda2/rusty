use rusty_components::NavMeshModifierComponent as Modifier;
use rusty_scene::nav_settings::MAX_AREAS;

pub fn set_active(m: &mut Modifier, active: bool) {
    m.active = active;
}

pub fn set_override_area(m: &mut Modifier, area: Option<i64>) {
    match area {
        None => m.override_area = None,
        Some(a) if (0..MAX_AREAS as i64).contains(&a) => m.override_area = Some(a as u8),
        Some(_) => {}
    }
}

pub fn set_ignore_from_build(m: &mut Modifier, ignore: bool) {
    m.ignore_from_build = ignore;
}

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
