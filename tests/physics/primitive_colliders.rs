//! A primitive made by `Scene.CreateEntity` is solid (#2051), as Unity's
//! `GameObject.CreatePrimitive` is: a ray finds a freshly created Box and Plane
//! with no `AddComponent` or `SetColliderShape`, and misses the Box once a script
//! removes its collider to make it draw-only.

use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

/// Tick 1: a ray where the Box will be, then a 1 m Box at z = 5 and a Plane at
/// y = -2. Tick 5: the ray again, and one down onto the Plane. Tick 6: the Box
/// loses its collider. Tick 10: the ray once more. Results land in the director's
/// Transform (box distance, plane distance, after-removal distance; -1 a miss).
const DIRECTOR: &str = "local n, box, plane, before = 0, nil, nil, 0\n\
local function ray(dy, dz)\n\
  local hit, id, d = Physics.Raycast(0, 0, 0, 0, dy, dz)\n\
  if not hit then return -1, nil end\n\
  return d, id end\n\
return { Update = function(id)\n\
  n = n + 1\n\
  if n == 1 then\n\
    before = ray(0, 1)\n\
    box = Scene.CreateEntity('Crate', 'Box')\n\
    Transform.SetPosition(box, 0, 0, 5)\n\
    plane = Scene.CreateEntity('Floor', 'Plane')\n\
    Transform.SetPosition(plane, 0, -2, 0)\n\
  end\n\
  if n == 5 then\n\
    local d, hit = ray(0, 1)\n\
    local p, on = ray(-1, 0)\n\
    if hit ~= box then d = -2 end\n\
    if on ~= plane then p = -2 end\n\
    Transform.SetPosition(id, d, p, before)\n\
  end\n\
  if n == 6 then Scene.RemoveComponent(box, 'Collider') end\n\
  if n == 10 then\n\
    local x, y = Transform.GetPosition(id)\n\
    Transform.SetPosition(id, x, y, (ray(0, 1)))\n\
  end\n\
end }";

#[test]
fn physics_raycast_hits_a_freshly_created_box_and_plane() {
    let mut scene = Scene::new();
    let director = scene.add_entity("Director".to_string());
    attach_script(&mut scene, director, "rusty_2051_director.lua", DIRECTOR);
    let scene = std::rc::Rc::new(std::cell::RefCell::new(scene));
    let mut game = playing(scene.clone(), 20.0);
    run(&mut game, 5);
    let at = |s: &std::rc::Rc<std::cell::RefCell<Scene>>| {
        s.borrow().world.transform(director).unwrap().position
    };
    let five = at(&scene);
    assert_eq!(five.z, -1.0, "nothing stood there before the Box was made");
    assert!(
        (five.x - 4.5).abs() < 0.05,
        "the Box's face at 4.5, got {}",
        five.x
    );
    assert!(
        (five.y - 2.0).abs() < 0.05,
        "the Plane 2 m down, got {}",
        five.y
    );
    run(&mut game, 5);
    assert_eq!(
        at(&scene).z,
        -1.0,
        "a Box without its collider is draw-only"
    );
}
