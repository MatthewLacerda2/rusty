//! `Physics.SetColliderShape(id, { kind = "Mesh", mesh = .. })` (#1960): a script
//! names a collider's collision mesh as a model's sidecar does, falling back to the
//! entity's own mesh when the model lacks the name; and `Assets.Get/SetModelImportSettings`'
//! `colliders`, the model inspector's sub-object rows.

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
    let kept = c.collision_mesh.expect("the name is kept");
    assert!(kept.indices.is_empty(), "and builds nothing");
    let full = format!("Physics.SetColliderShape({id}, {{kind='Mesh', mesh='{path}::Chair_col'}})");
    m.eval(&full).unwrap();
    assert_eq!(triangles(), 2, "a full reference works too");
    m.eval(&format!("Physics.SetColliderShape({id}, {{kind='Mesh'}})"))
        .unwrap();
    assert_eq!(triangles(), 50, "no name: the own mesh again");
}

#[test]
fn the_assets_api_writes_a_sub_objects_collider() {
    let (m, _, _) = manager_with_collider();
    let path = chair("rusty_1960_lua_sidecar.obj", 2, None);
    m.exec(&format!(
        "local p = {path:?}
         local c = Assets.GetModelImportSettings(p).colliders
         assert(c.Chair.kind == 'TriMesh' and c.Chair.mesh == nil)
         Assets.SetModelImportSettings(p, {{ colliders = {{
             Chair = {{ kind = 'Convex', mesh = 'Chair_col' }},
             Chair_col = {{ kind = 'TriMesh', mesh = 'Chair_col' }} }} }})
         c = Assets.GetModelImportSettings(p).colliders
         assert(c.Chair.kind == 'Convex' and c.Chair.mesh == 'Chair_col')
         assert(c.Chair_col.mesh == nil, 'naming itself is its own')
         Assets.SetModelImportSettings(p, {{ colliders = {{ Chair_col = false }} }})
         c = Assets.GetModelImportSettings(p).colliders
         assert(c.Chair_col == nil and c.Chair.mesh == 'Chair_col', 'only the keys given')"
    ))
    .unwrap();
    let bad = format!("Assets.SetModelImportSettings({path:?}, {{ colliders = {{ Chair = {{ kind = 'Box' }} }} }})");
    assert!(m.exec(&bad).is_err());
}

#[test]
fn a_name_on_a_mesh_from_no_model_is_kept() {
    use crate::scene::authoring::{add_component, ComponentKind};
    let (m, scene, id) = manager_with_collider();
    assert!(add_component(
        &mut scene.borrow_mut(),
        id,
        ComponentKind::Mesh
    ));
    let set = format!("Physics.SetColliderShape({id}, {{kind='Mesh', mesh='Chair_col'}})");
    m.eval(&set).unwrap();
    let read = format!("return Physics.GetColliderShape({id}).mesh");
    assert_eq!(
        m.eval(&read).unwrap().trim(),
        "Chair_col",
        "kept, as Physics.md says"
    );
    let s = scene.borrow();
    assert_eq!(
        collider_world_triangles(&s, id).map_or(0, |t| t.triangles.len()),
        12
    );
}

#[test]
fn collision_meshes_naming_each_other_are_refused_and_nothing_is_written() {
    let (m, _, _) = manager_with_collider();
    let path = chair("rusty_1960_lua_loop.obj", 2, Some("Chair_col"));
    let meta = crate::asset::sidecar::meta_path(std::path::Path::new(&path));
    let before = std::fs::read_to_string(&meta).unwrap();
    let looped = format!(
        "Assets.SetModelImportSettings({path:?}, {{ scaleFactor = 2, colliders = {{
             Chair_col = {{ kind = 'TriMesh', mesh = 'Chair' }} }} }})"
    );
    let err = m.exec(&looped).unwrap_err().to_string();
    assert!(err.contains("name each other"), "{err}");
    assert_eq!(
        std::fs::read_to_string(&meta).unwrap(),
        before,
        "nothing on disk"
    );
}
