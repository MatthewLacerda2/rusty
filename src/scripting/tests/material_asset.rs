//! src/scripting/tests/material_asset.rs — the physics material asset verbs
//! (#1878): `Physics.CreateMaterialAsset` / `GetMaterialAsset` / `SetMaterialAsset`,
//! `SetMaterial` / `GetMaterial`, and the collision contact naming the other
//! collider's asset.

use std::collections::BTreeMap;

use super::collider::manager_with_collider;
use super::lifecycle::write_script;
use crate::physics::{CollisionEvents, CollisionPair, Contact, PhysicsEvents};
use crate::scene::authoring::defaults::default_collider;

fn asset(name: &str) -> String {
    let path = crate::test_temp::dir().join(name);
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn an_asset_is_created_named_and_retuned_from_lua() {
    let (m, _scene, id) = manager_with_collider();
    let ice = asset("ice.physmat");
    let lua = format!(
        "Physics.CreateMaterialAsset('{ice}', {{friction = 0.25, bounce_combine = 'maximum'}})\n\
         Physics.SetMaterial({id}, '{ice}')\n\
         local before = select(1, Physics.GetPhysicsMaterial({id}))\n\
         Physics.SetMaterialAsset('{ice}', {{friction = 0.5}})\n\
         local a = Physics.GetMaterialAsset('{ice}')\n\
         return Physics.GetMaterial({id}) == '{ice}', before, \
           select(1, Physics.GetPhysicsMaterial({id})), a.bounce_combine, a.bounciness"
    );
    assert_eq!(m.eval(&lua).unwrap(), "true, 0.25, 0.5, Maximum, 0");
    // Unity's `Collider.material`: inline values detach the collider from the asset.
    m.eval(&format!("Physics.SetPhysicsMaterial({id}, 0.75, 0)"))
        .unwrap();
    assert_eq!(
        m.eval(&format!("return Physics.GetMaterial({id})"))
            .unwrap(),
        "nil"
    );
    m.eval(&format!(
        "Physics.SetMaterial({id}, '{ice}'); Physics.SetMaterial({id}, nil)"
    ))
    .unwrap();
    let inline = m.eval(&format!("return Physics.GetPhysicsMaterial({id})"));
    assert_eq!(inline.unwrap(), "0.75, 0, Average, Average");
}

#[test]
fn the_verbs_refuse_what_unity_would() {
    let (m, _scene, id) = manager_with_collider();
    let wood = asset("wood.physmat");
    let missing = asset("missing.physmat");
    m.eval(&format!("Physics.CreateMaterialAsset('{wood}')"))
        .unwrap();
    for (lua, why) in [
        (
            format!("Physics.CreateMaterialAsset('{wood}')"),
            "already exists",
        ),
        (
            format!("Physics.SetMaterial({id}, '{missing}')"),
            "no physics material",
        ),
        (
            format!("Physics.SetMaterialAsset('{missing}', {{}})"),
            "no physics material",
        ),
        (
            format!("Physics.SetMaterialAsset('{wood}', {{friction = 0/0}})"),
            "finite",
        ),
        (
            format!("Physics.SetMaterialAsset('{wood}', {{friction_combine = 'x'}})"),
            "combine",
        ),
        (
            format!("Physics.CreateMaterialAsset('{}')", asset("wood.json")),
            ".physmat",
        ),
    ] {
        let e = m.eval(&lua).unwrap_err().to_string();
        assert!(e.contains(why), "{lua}: {e}");
    }
    assert_eq!(
        m.eval(&format!("return Physics.GetMaterialAsset('{missing}')"))
            .unwrap(),
        "nil"
    );
}

#[test]
fn a_contact_names_the_other_colliders_asset() {
    let (mut m, scene, id) = manager_with_collider();
    let metal = asset("metal.physmat");
    m.eval(&format!("Physics.CreateMaterialAsset('{metal}')"))
        .unwrap();
    let other = {
        let mut s = scene.borrow_mut();
        let other = s.add_entity("Plate".to_string());
        let collider = crate::components::ColliderComponent {
            material_asset: Some(metal.clone()),
            ..default_collider()
        };
        s.world.set_collider(other, Some(collider));
        other
    };
    let code = "_G.__m = {}\nreturn { OnCollisionEnter = function(id, other, c)\n\
                table.insert(__m, tostring(c.material)) end }";
    let path = write_script("contact", code);
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts();
    let contact = Contact {
        point: glam::Vec3::ZERO,
        normal: glam::Vec3::Y,
        relative_velocity: glam::Vec3::ZERO,
        impulse: 1.0,
        other_body: other,
    };
    let pair = |b| CollisionPair {
        a: id,
        b,
        contact,
        body_a: id,
    };
    m.dispatch_physics_events(PhysicsEvents {
        collisions: CollisionEvents {
            entered: vec![pair(other), pair(other + 1)],
            ..Default::default()
        },
        ..Default::default()
    });
    let seen = m.eval("return table.concat(__m, ',')").unwrap();
    assert_eq!(
        seen,
        format!("{metal},nil"),
        "an entity with no asset names none"
    );
}
