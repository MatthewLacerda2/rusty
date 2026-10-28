//! Persistent decals re-attached by hierarchy path (#1518) when names repeat or
//! hold a `/`: each owner gets its own marks back after a reload.

use glam::Vec3;

use crate::scene::decal::DecalSpec;
use crate::scene::{to_scene_data, Scene};

/// A path with no lighting sidecar beside it: the swap's sidecar merge is a no-op.
const NO_SIDECAR: &str = "rusty_1518_no_such_dir/level.scene";

/// `Arena` with one child per name, child `i` at `x = i`; returns the children.
fn arena(s: &mut Scene, names: &[&str]) -> Vec<u32> {
    let arena = s.add_entity("Arena".into());
    let children = names.iter().enumerate().map(|(i, name)| {
        let child = s.add_entity((*name).into());
        s.set_parent(child, Some(arena)).unwrap();
        s.world.transform_mut(child).unwrap().position = Vec3::X * i as f32;
        child
    });
    children.collect()
}

/// Reload the same arena, with a `Player` kept so the incoming ids shift.
fn reload(s: &mut Scene, names: &[&str]) {
    let mut level = Scene::new();
    arena(&mut level, names);
    let survivors = s.load_survivors();
    s.swap_in(to_scene_data(&level), NO_SIDECAR, &survivors)
        .unwrap();
}

/// Mark each owner where it stands with one persistent decal, reload with the
/// owners shuffled to new ids, and return each decal's world centre in spawn order.
fn marks_after_reload(names: &[&str]) -> Vec<Vec3> {
    let mut s = Scene::new();
    let player = s.add_entity("Player".into());
    s.dont_destroy_on_load(player);
    for (i, owner) in arena(&mut s, names).into_iter().enumerate() {
        let spec = DecalSpec {
            owner: Some(owner),
            persist: true,
            ..DecalSpec::default()
        };
        s.spawn_decal(Vec3::X * i as f32, Vec3::Y, spec).unwrap();
    }
    reload(&mut s, names);
    let centre = |i: usize| s.decal_pose(&s.decals[i]).unwrap().centre();
    (0..s.decals.len()).map(centre).collect()
}

#[test]
fn five_same_named_siblings_each_keep_their_own_marks() {
    let marks = marks_after_reload(&["Crate"; 5]);
    let want: Vec<Vec3> = (0..5).map(|i| Vec3::X * i as f32).collect();
    assert_eq!(marks, want, "each mark back on its own crate");
}

#[test]
fn a_name_holding_a_slash_round_trips() {
    let marks = marks_after_reload(&["a", "a/b", "b"]);
    assert_eq!(marks, [Vec3::ZERO, Vec3::X, Vec3::X * 2.0]);
}
