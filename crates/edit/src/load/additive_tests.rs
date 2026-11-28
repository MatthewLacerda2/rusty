use super::join_prepared;
use crate::load::prepare::PreparedScene;
use crate::to_scene_data;
use rusty_components::{JointComponent, MaterialAsset};
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::Scene;

/// No navmesh or lighting sidecar beside this path, so the join reads none.
const NO_SIDECAR: &str = "rusty_2499_no_such_dir/level.scene";

/// A World already holding three entities, ids 1 to 3.
fn running() -> Scene {
    let mut scene = Scene::new();
    for name in ["Player", "Gun", "Hud"] {
        scene.add_entity(name.to_string());
    }
    scene
}

/// A door (id 1) hinged on its frame (id 2), the frame's child a sign (id 3).
fn piece() -> Scene {
    let mut piece = Scene::new();
    let door = piece.add_entity("Door".to_string());
    let frame = piece.add_entity("Frame".to_string());
    let sign = piece.add_entity("Sign".to_string());
    piece.set_parent(sign, Some(frame)).unwrap();
    let hinge = JointComponent {
        connected_body: Some(frame),
        ..Default::default()
    };
    piece.world.set_joint(door, Some(hinge));
    piece
}

fn join(scene: &mut Scene, piece: &Scene) -> super::Joined {
    let data = to_scene_data(piece);
    join_prepared(scene, PreparedScene::from_data(data, NO_SIDECAR))
}

#[test]
fn every_incoming_id_moves_past_the_worlds_and_its_references_follow() {
    let mut scene = running();
    let before = scene.world.next_id();
    let joined = join(&mut scene, &piece());
    let id = |name: &str| scene.find_entity_by_name(name).unwrap();
    let (door, frame, sign) = (id("Door"), id("Frame"), id("Sign"));
    assert_eq!(
        [door, frame, sign],
        [before, before + 1, before + 2],
        "document order"
    );
    assert_eq!(joined.roots, [door, frame]);
    assert_eq!(scene.world.parent_id(sign), Some(frame));
    let hinge = scene.world.joint(door).and_then(|j| j.connected_body);
    assert_eq!(hinge, Some(frame), "the hinge names the moved frame");
    assert!(
        scene.find_entity_by_name("Player").is_some(),
        "nothing loaded was touched"
    );
    let again = join(&mut scene, &piece());
    assert!(
        again.ids.iter().all(|&i| i > sign),
        "a second copy takes new ids again"
    );
}

#[test]
fn a_material_of_a_taken_name_is_renamed_and_an_equal_one_shared() {
    let mut scene = running();
    let red = MaterialAsset::default();
    scene.materials.insert("Paint".into(), red.clone());
    let mut piece = piece();
    let mut blue = red.clone();
    blue.metallic = 0.75;
    piece.materials.insert("Paint".into(), blue);
    piece.materials.insert("Same".into(), red.clone());
    scene.materials.insert("Same".into(), red);
    join(&mut scene, &piece);
    assert!(
        scene.materials.contains_key("Paint (1)"),
        "the incoming paint, renamed"
    );
    assert_eq!(scene.materials.len(), 3, "the equal one is shared");
}

#[test]
fn its_bakes_are_named_and_a_layer_mismatch_is_reported() {
    let mut scene = running();
    let mut piece = piece();
    piece.lightmaps.insert(LightmapEntry {
        entity: 1,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    piece.layers.set_name(9, "Water");
    let joined = join(&mut scene, &piece);
    assert_eq!(joined.skipped, ["lightmaps"]);
    assert!(joined.disagrees);
    assert!(scene.lightmaps.is_empty(), "no lightmap is applied");
    assert_eq!(
        scene.layers.name(9),
        running().layers.name(9),
        "the World's stay"
    );
}
