//! crates/navigation/src/object_modifier.rs — how the bake reads one collider, after the
//! `NavMeshModifier`s on it and above it (#1959).
//!
//! The modifier that covers an entity is the nearest active one on the entity
//! itself or, with `apply_to_children`, on an ancestor (Unity's rule: a child's own
//! modifier wins over its parent's, whole). An inactive modifier covers nothing and
//! does not shield what is below it from one further up.

use super::areas::clamp_area;
use rusty_components::NavMeshModifierComponent;
use rusty_ecs::World;

/// What the covering modifier says about a collider's entity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BakeAs {
    /// The collider is left out of the bake.
    pub ignored: bool,
    /// The area its walkable tops bake as, `None` for the bake's own.
    pub area: Option<u8>,
}

/// What covers entity `id`, or the default (built, no override) when nothing does.
pub(super) fn bake_as(world: &World, id: u32) -> BakeAs {
    let covers = |(i, e): (usize, u32)| {
        let m = world.nav_mesh_modifier(e)?;
        (m.active && (i == 0 || m.apply_to_children)).then(|| of(&m))
    };
    world
        .self_and_ancestors(id)
        .enumerate()
        .find_map(covers)
        .unwrap_or_default()
}

fn of(m: &NavMeshModifierComponent) -> BakeAs {
    BakeAs {
        ignored: m.ignore_from_build,
        area: m.override_area.map(clamp_area),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifier(area: Option<u8>, children: bool) -> NavMeshModifierComponent {
        NavMeshModifierComponent {
            override_area: area,
            apply_to_children: children,
            ..Default::default()
        }
    }

    #[test]
    fn the_nearest_active_covering_modifier_wins() {
        let mut w = World::new();
        let [root, mid, leaf] = ["root", "mid", "leaf"].map(|n| w.spawn(n.into()));
        w.set_parent_id(mid, Some(root));
        w.set_parent_id(leaf, Some(mid));
        assert_eq!(bake_as(&w, leaf), BakeAs::default(), "nothing covers it");
        w.set_nav_mesh_modifier(root, Some(modifier(Some(5), true)));
        assert_eq!(bake_as(&w, leaf).area, Some(5), "inherited from the root");
        w.set_nav_mesh_modifier(mid, Some(modifier(Some(1), false)));
        assert_eq!(bake_as(&w, mid).area, Some(1), "its own");
        assert_eq!(bake_as(&w, leaf).area, Some(5), "mid's skips its children");
        w.set_nav_mesh_modifier(mid, Some(modifier(Some(1), true)));
        assert_eq!(bake_as(&w, leaf).area, Some(1), "the nearer one wins");
        w.nav_mesh_modifier_mut(mid).unwrap().active = false;
        assert_eq!(
            bake_as(&w, leaf).area,
            Some(5),
            "an inactive one shields nothing"
        );
        w.set_nav_mesh_modifier(leaf, Some(modifier(Some(255), true)));
        assert_eq!(
            bake_as(&w, leaf).area,
            Some((crate::MAX_AREAS - 1) as u8),
            "a loaded id past the table"
        );
    }
}
