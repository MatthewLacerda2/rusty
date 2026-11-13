//! A scene load re-roots a survivor (#432): the nav bake sees its collider where
//! it stands now, not under the holder it left. `scene`'s own half (the collider's
//! bounds) is `scene::runtime::load_tests`.

use super::NavigationGraph;
use crate::scene::{authoring, to_scene_data, Scene};

/// A path with no lighting sidecar beside it: the swap's sidecar merge is a no-op.
const NO_SIDECAR: &str = "rusty_432_no_such_dir/level.scene";

#[test]
fn a_re_rooted_survivor_is_baked_where_it_stands_now() {
    let mut s = Scene::new();
    let holder = s.add_entity("Holder".to_string());
    let crate_ = s.add_entity("Crate".to_string());
    s.world.transform_mut(holder).unwrap().position = glam::Vec3::X * 100.0;
    s.set_parent(crate_, Some(holder)).unwrap();
    s.world.set_static(crate_, true);
    let collider = authoring::defaults::default_collider();
    s.world.set_collider(crate_, Some(collider));
    s.update_all_colliders();
    s.dont_destroy_on_load(crate_);

    let mut level = Scene::new();
    let floor = level.add_entity("Floor".to_string());
    let lamp = level.add_entity("Lamp".to_string());
    level.set_parent(lamp, Some(floor)).unwrap();
    let survivors = s.load_survivors();
    s.swap_in(to_scene_data(&level), NO_SIDECAR, &survivors)
        .unwrap();
    let (min, _) = s.world.collider_bounds(crate_).unwrap();
    let nav = NavigationGraph::from_scene(&s).bounds();
    assert!(nav.min_x <= min.x && nav.max_x < 50.0, "{nav:?}");
}
