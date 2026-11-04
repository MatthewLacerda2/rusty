//! Scene edits during Play that change what physics carries, run through the
//! real play loop (#1707): physics syncs only what the change log names, so these
//! are the moves it must still see. A compound body loses a part, a body is
//! re-parented onto a moving one, and a parent's `SetActive` switches off the wall
//! under it. A ragdoll at rest losing its floor the same way: `ragdoll::floor_off`.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::{default_collider, default_rigidbody};
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

/// An entity named `name` at `at` with a `size` box collider.
fn boxed(scene: &mut Scene, name: &str, at: Vec3, size: Vec3) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.transform_mut(id).unwrap().position = at;
    let shape = ColliderShape::Box { size };
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// Tick 60: a ray down where the cart's mast stands. Tick 61: the mast loses its
/// collider. Tick 62: the same ray. Tick 70: the hat is parented to the cart.
/// Tick 71: the gate goes inactive and the cart is shoved at the wall behind it.
const DIRECTOR: &str = "local n = 0\n\
local function find(name) return Scene.FindEntityByName(name) end\n\
local function ray()\n\
  local cx = Transform.GetPosition(find('Cart'))\n\
  local hit, _, d = Physics.Raycast(cx + 0.8, 5, 0, 0, -1, 0)\n\
  return hit and d or -1 end\n\
local before = 0\n\
return { Update = function(id)\n\
  n = n + 1\n\
  if n == 60 then before = ray() end\n\
  if n == 61 then Scene.RemoveComponent(find('Mast'), 'Collider') end\n\
  if n == 62 then Transform.SetPosition(id, before, ray(), 0) end\n\
  if n == 70 then Scene.SetParent(find('Hat'), find('Cart')) end\n\
  if n == 71 then\n\
    Scene.SetActive(find('Gate'), false)\n\
    Physics.SetVelocity(find('Cart'), 8, 0, 0)\n\
  end\n\
end }";

#[test]
fn a_cart_loses_its_mast_takes_a_hat_and_drives_through_a_switched_off_wall() {
    let mut scene = Scene::new();
    let floor = boxed(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(40.0, 1.0, 40.0),
    );
    scene.world.set_static(floor, true);
    let cart = boxed(
        &mut scene,
        "Cart",
        Vec3::new(0.0, 0.25, 0.0),
        Vec3::new(2.0, 0.5, 1.0),
    );
    scene.world.set_rigidbody(cart, Some(default_rigidbody()));
    let mast = boxed(
        &mut scene,
        "Mast",
        Vec3::new(0.8, 1.25, 0.0),
        Vec3::new(0.2, 2.0, 0.2),
    );
    scene.set_parent(mast, Some(cart)).unwrap();
    let hat = boxed(
        &mut scene,
        "Hat",
        Vec3::new(0.0, 1.75, 0.0),
        Vec3::splat(0.5),
    );
    let gate = scene.add_entity("Gate".to_string());
    let wall = boxed(
        &mut scene,
        "Wall",
        Vec3::new(1.5, 1.0, 0.0),
        Vec3::new(0.2, 2.0, 4.0),
    );
    scene.set_parent(wall, Some(gate)).unwrap();
    scene.world.set_static(wall, true);
    let director = scene.add_entity("Director".to_string());
    attach_script(&mut scene, director, "rusty_1707_director.lua", DIRECTOR);
    let scene = std::rc::Rc::new(std::cell::RefCell::new(scene));
    let mut game = playing(scene.clone(), 20.0);
    run(&mut game, 180);
    let s = scene.borrow();
    let rays = s.world.transform(director).unwrap().position;
    assert!(
        (rays.x - 2.5).abs() < 0.05,
        "the ray stopped on the mast's top: {rays}"
    );
    assert!(
        (rays.y - 4.5).abs() < 0.05,
        "then passed where it stood: {rays}"
    );
    let (c, h) = (s.world.world_position(cart), s.world.world_position(hat));
    assert!(
        c.x > 3.0,
        "the cart drove through the switched-off wall: {c}"
    );
    assert!(
        (h.x - c.x).abs() < 0.05,
        "the hat rode on the cart: {h} over {c}"
    );
    assert!(h.y > c.y + 1.0, "and stayed on top of it: {h} over {c}");
}
