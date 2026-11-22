use glam::Vec3;

use crate::{Activity, Changes, Cursor, World};
use rusty_components::{
    ColliderComponent, Entity, MeshComponent, NavMeshModifierComponent, TextComponent,
};

struct Cursors {
    collider: Cursor<ColliderComponent>,
    mesh: Cursor<MeshComponent>,
    text: Cursor<TextComponent>,
    modifier: Cursor<NavMeshModifierComponent>,
    activity: Cursor<Activity>,
}

impl Cursors {
    fn open(w: &World) -> Self {
        Self {
            collider: w.change_cursor(),
            mesh: w.change_cursor(),
            text: w.change_cursor(),
            modifier: w.change_cursor(),
            activity: w.change_cursor(),
        }
    }

    fn read(&mut self, w: &World) -> [Changes; 5] {
        [
            w.changes(&mut self.collider),
            w.changes(&mut self.mesh),
            w.changes(&mut self.text),
            w.changes(&mut self.modifier),
            w.changes(&mut self.activity),
        ]
    }
}

fn casing(id: u32) -> Entity {
    let mut doc = Entity::new(id, "casing".into());
    doc.collider = Some(ColliderComponent::from_mesh_bounds(
        false,
        Vec3::ZERO,
        Vec3::ONE,
    ));
    doc.mesh = Some(MeshComponent::new("Casing", Vec::new(), Vec::new()));
    doc
}

fn ids(of: &[u32]) -> Changes {
    Changes::Ids(of.to_vec())
}

#[test]
fn a_placed_entity_is_stamped_on_its_columns_and_no_other() {
    let mut w = World::new();
    let mut c = Cursors::open(&w);
    let id = w.next_id();
    w.insert_entity(casing(id));
    let none = ids(&[]);
    let [collider, mesh, text, modifier, activity] = c.read(&w);
    assert_eq!(
        [collider, mesh],
        [ids(&[id]), ids(&[id])],
        "what it carries"
    );
    assert_eq!([text, modifier], [none.clone(), none], "what it lacks");
    assert_eq!(activity, ids(&[id]), "every placement");
}

#[test]
fn a_removed_entity_is_stamped_on_the_columns_it_had() {
    let mut w = World::new();
    let id = w.next_id();
    w.insert_entity(casing(id));
    let mut c = Cursors::open(&w);
    w.despawn(id);
    let none = ids(&[]);
    let [collider, mesh, text, modifier, activity] = c.read(&w);
    assert_eq!([collider, mesh], [ids(&[id]), ids(&[id])], "what it had");
    assert_eq!([text, modifier], [none.clone(), none], "what it never had");
    assert_eq!(activity, ids(&[id]));
}

#[test]
fn an_overwrite_stamps_the_columns_it_dropped_and_the_ones_it_gained() {
    let mut w = World::new();
    let id = w.next_id();
    w.insert_entity(casing(id));
    let mut c = Cursors::open(&w);
    let mut doc = w.entity_document(id).expect("live");
    doc.collider = None;
    doc.text = Some(TextComponent::default());
    w.replace_entity(doc);
    let [collider, mesh, text, modifier, _] = c.read(&w);
    assert_eq!(collider, ids(&[id]), "the dropped column");
    assert_eq!(mesh, ids(&[id]), "a column both rows carry");
    assert_eq!(text, ids(&[id]), "the gained column");
    assert_eq!(modifier, ids(&[]), "a column neither row carries");
}

#[test]
fn a_restored_entity_is_stamped_on_its_columns() {
    let mut w = World::new();
    let id = w.next_id();
    let mut doc = casing(id);
    doc.nav_mesh_modifier = Some(NavMeshModifierComponent::default());
    w.insert_entity(doc.clone());
    let seq = w.seq(id).expect("live");
    w.despawn(id);
    let mut c = Cursors::open(&w);
    w.restore_entity(doc, seq);
    let [collider, _, text, modifier, _] = c.read(&w);
    assert_eq!([collider, modifier], [ids(&[id]), ids(&[id])]);
    assert_eq!(text, ids(&[]));
}
