//! The fingerprint holds no entity id and no visiting order (#2127): a scene whose
//! ids were renumbered (a `DontDestroyOnLoad` survivor moving an incoming id) still
//! matches the file it was baked to, and its authored links re-snap under the new ids.

use glam::Vec3;

use super::super::super::super::test_support::{add_box, add_floor, add_obstacle, move_to};
use super::super::super::super::NavigationGraph;
use super::{baked, saved};
use crate::components::OffMeshLinkComponent;
use crate::scene::Scene;

/// A floor, two crates, a carving obstacle and an authored link, in an order that
/// gives each different ids when `reversed`, after `pad` empty entities. Returns the
/// link's id and the first crate's (the one at 2..3).
fn level(pad: usize, reversed: bool) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    scene.nav_settings.agent_radius = 0.25;
    (0..pad).for_each(|_| drop(scene.add_entity("Survivor's id".into())));
    let crates = [(2.0, 3.0), (5.0, 6.0)];
    let order: Vec<_> = match reversed {
        true => crates.iter().rev().collect(),
        false => crates.iter().collect(),
    };
    let boxes = |s: &mut Scene| {
        let ids: Vec<_> = order
            .iter()
            .map(|&&(lo, hi)| add_box(s, Vec3::new(lo, 0.0, lo), Vec3::new(hi, 1.0, hi)))
            .collect();
        ids[usize::from(reversed)]
    };
    let link = |s: &mut Scene| {
        let id = s.add_entity("Link".into());
        let link = OffMeshLinkComponent {
            start: Vec3::new(1.0, 0.0, 6.0),
            end: Vec3::new(7.0, 0.0, 1.0),
            ..Default::default()
        };
        s.world.set_offmesh_link(id, Some(link));
        id
    };
    let (link_id, crate_id) = if reversed {
        let id = link(&mut scene);
        let crate_id = boxes(&mut scene);
        add_floor(&mut scene, 0.0, 8.0, 0.0, 8.0);
        (id, crate_id)
    } else {
        add_floor(&mut scene, 0.0, 8.0, 0.0, 8.0);
        let crate_id = boxes(&mut scene);
        (link(&mut scene), crate_id)
    };
    add_obstacle(&mut scene, Vec3::new(4.0, 0.5, 2.0), Vec3::ONE, true);
    (scene, link_id, crate_id)
}

fn links(g: &NavigationGraph) -> Vec<String> {
    g.offmesh_links().map(|l| format!("{l:?}")).collect()
}

#[test]
fn renumbered_ids_keep_the_file_current() {
    let (written, ..) = level(0, false);
    let (_, bytes) = saved(&written);
    for (pad, reversed) in [(1, false), (3, true), (0, true)] {
        let (scene, link_id, _) = level(pad, reversed);
        let mut g = NavigationGraph::from_scene(&scene);
        let at = format!("pad {pad}, reversed {reversed}");
        g.restore(&scene, &bytes)
            .unwrap_or_else(|e| panic!("{at}: {e}"));
        assert_eq!(
            format!("{:?}", g.spans),
            format!("{:?}", baked(&scene).spans)
        );
        assert_eq!(links(&g), links(&baked(&scene)), "{at}");
        let owners: Vec<_> = g.offmesh.authored.iter().map(|l| l.owner).collect();
        assert_eq!(
            owners,
            vec![Some(link_id)],
            "{at}: re-snapped under its new id"
        );
    }
}

#[test]
fn a_moved_collider_under_renumbered_ids_is_still_refused() {
    let (written, ..) = level(0, false);
    let (_, bytes) = saved(&written);
    let (mut scene, _, crate_id) = level(2, true);
    move_to(&mut scene, crate_id, Vec3::new(1.0, 0.5, 6.5));
    let mut g = NavigationGraph::from_scene(&scene);
    assert!(g.restore(&scene, &bytes).is_err(), "a crate moved");
}
