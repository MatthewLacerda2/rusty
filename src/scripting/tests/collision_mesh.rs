//! `Physics.SetColliderShape(id, { kind = "Mesh", mesh = .. })` (#1960): a script
//! names a collider's collision mesh as a model's sidecar does, falling back to the
//! entity's own mesh when the model lacks the name.

use super::collider::manager_with_collider;
use crate::physics::collider_world_triangles;
use crate::scene::asset_instance::collision::tests::chair;
use crate::scene::asset_instance::instantiate_asset;

#[test]
fn a_script_names_the_collision_mesh() {
    let (m, scene, _) = manager_with_collider();
    let path = chair("rusty_1960_lua_chair.obj", 5, None);
    let reference = format!("{path}::Chair");
    let id = instantiate_asset(&mut scene.borrow_mut(), &reference, None, glam::Vec3::ZERO);
    let id = id.unwrap();
    let triangles = || {
        let s = scene.borrow();
        collider_world_triangles(&s, id).map_or(0, |t| t.triangles.len())
    };
    assert_eq!(triangles(), 50, "the sidecar's own mesh");

    m.eval(&format!(
        "Physics.SetColliderShape({id}, {{kind='Mesh', mesh='Chair_col'}})"
    ))
    .unwrap();
    assert_eq!(
        triangles(),
        2,
        "a bare name is a sub-object of its own model"
    );
    let read = m
        .eval(&format!(
            "local s = Physics.GetColliderShape({id}); return s.kind .. ' ' .. s.mesh"
        ))
        .unwrap();
    assert_eq!(read.trim(), format!("Mesh {path}::Chair_col"));

    m.eval(&format!(
        "Physics.SetColliderShape({id}, {{kind='Mesh', convex=true, mesh='Nope'}})"
    ))
    .unwrap();
    assert!(triangles() > 2, "a missing name falls back to the own mesh");
    let c = scene.borrow().world.collider(id).unwrap().clone();
    assert_eq!(c.shared_mesh, None);
    let full = format!("Physics.SetColliderShape({id}, {{kind='Mesh', mesh='{path}::Chair_col'}})");
    m.eval(&full).unwrap();
    assert_eq!(triangles(), 2, "a full reference works too");
    m.eval(&format!("Physics.SetColliderShape({id}, {{kind='Mesh'}})"))
        .unwrap();
    assert_eq!(triangles(), 50, "no name: the own mesh again");
}
