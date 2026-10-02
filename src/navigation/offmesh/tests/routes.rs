//! Paths across links: a drop is taken when it is the short way, never climbed
//! back up, and a ladder joins two floors.

use glam::Vec3;

use super::*;
use crate::components::{NavPathStatus, OffMeshLinkData};

#[test]
fn a_drop_is_taken_when_it_is_the_shortest_route() {
    let mut scene = platform();
    let (top, floor) = (Vec3::new(4.0, 3.0, 4.0), Vec3::new(14.0, 0.0, 4.0));
    let with = baked(&scene).calculate_path(top, floor);
    assert_eq!(with.status, NavPathStatus::Complete);
    assert_eq!(with.links.len(), 1, "{:?}", with.corners);
    let (i, drop) = with.links[0];
    assert_eq!(drop.kind, OffMeshLinkKind::Drop);
    assert_eq!(
        (with.corners[i - 1], with.corners[i]),
        (drop.start, drop.end)
    );

    scene.nav_settings.drop_height = 0.0;
    let around = baked(&scene).calculate_path(top, floor);
    assert_eq!(around.status, NavPathStatus::Complete, "down the ramp");
    assert!(around.links.is_empty());
    assert!(with.length() + 5.0 < around.length());
}

#[test]
fn a_one_way_drop_is_never_climbed_back_up() {
    let g = baked(&platform());
    let up = g.calculate_path(Vec3::new(14.0, 0.0, 4.0), Vec3::new(4.0, 3.0, 4.0));
    assert_eq!(up.status, NavPathStatus::Complete, "up the ramp");
    // A jump onto the ramp's low side (under 1 m) is fine; nothing climbs 3 m.
    let climbs = |l: &OffMeshLinkData| l.end.y - l.start.y > 1.2;
    assert!(!up.links.iter().any(|(_, l)| climbs(l)), "{:?}", up.links);
    assert!(
        up.corners.iter().any(|c| c.z > 8.0),
        "goes round by the ramp"
    );
}

#[test]
fn an_authored_ladder_connects_two_floors() {
    let (mut scene, ladder) = ladder();
    let (ground, deck) = (Vec3::new(2.0, 0.0, 5.0), Vec3::new(16.0, 4.0, 5.0));
    let g = baked(&scene);
    assert!(g.link_connected(ladder));
    let up = g.calculate_path(ground, deck);
    assert_eq!(up.status, NavPathStatus::Complete);
    let (_, link) = up.links[0];
    assert_eq!(link.kind, OffMeshLinkKind::Manual);
    assert_eq!(link.owner, Some(ladder));
    assert_eq!((link.start.y, link.end.y), (0.0, 4.0));
    let down = g.calculate_path(deck, ground);
    assert_eq!(down.status, NavPathStatus::Complete, "two-way by default");
    assert_eq!(down.links[0].1.end.y, 0.0);

    scene.world.offmesh_link_mut(ladder).unwrap().active = false;
    let g = baked(&scene);
    assert!(!g.link_connected(ladder));
    assert_eq!(
        g.calculate_path(ground, deck).status,
        NavPathStatus::Partial
    );
}

#[test]
fn smoothing_never_pulls_a_string_across_a_link() {
    let g = baked(&ladder().0);
    let path = g.calculate_path(Vec3::new(2.0, 0.0, 2.0), Vec3::new(16.0, 4.0, 8.0));
    let (i, link) = path.links[0];
    assert_eq!(
        path.corners[i - 1],
        link.start,
        "walks to the ladder's foot"
    );
    assert_eq!(path.corners[i], link.end, "and leaves from its top");
}

#[test]
fn an_agent_never_climbs_a_ladder_its_area_mask_excludes() {
    let (mut scene, ladder) = ladder();
    scene.world.offmesh_link_mut(ladder).unwrap().area = 3;
    let (ground, deck) = (Vec3::new(2.0, 0.0, 5.0), Vec3::new(16.0, 4.0, 5.0));
    let g = baked(&scene);
    assert_eq!(
        g.calculate_path(ground, deck).status,
        NavPathStatus::Complete
    );
    let no_ladders = !(1 << 3);
    let path = g.calculate_path_masked(ground, deck, no_ladders);
    assert_eq!(path.status, NavPathStatus::Partial, "stays on the ground");
    assert!(path.links.is_empty());
}
