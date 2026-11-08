//! Physics material assets on hits (#1878): a shot through a wood box into a
//! concrete box reads each crossing's `material` and `thickness`, and prices the
//! damage left from a Lua table keyed by material, the engine storing no
//! penetration value of its own.
//!
//! Along +z from the origin: 0.5 m of wood centred at z = 3, then 1 m of concrete
//! centred at z = 10, and an inline-material box at z = 20.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::default_collider;
use rusty::scene::Scene;

use crate::play::{playing, run};

fn slab(scene: &mut Scene, z: f32, depth: f32, asset: Option<&str>) -> u32 {
    let id = scene.add_entity("Slab".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 0.0, z);
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        shape: ColliderShape::Box {
            size: Vec3::new(2.0, 2.0, depth),
        },
        material_asset: asset.map(str::to_string),
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// The range in Play, its two assets written a tick after its colliders were
/// built, then one more tick: (`game`, wood
/// path, concrete path).
fn range() -> (GameWorld, String, String) {
    let dir = crate::temp::dir();
    let path = |n: &str| dir.join(n).to_string_lossy().replace('\\', "/");
    let (wood, concrete) = (path("wood.physmat"), path("concrete.physmat"));
    let mut s = Scene::new();
    slab(&mut s, 3.0, 0.5, Some(&wood));
    slab(&mut s, 10.0, 1.0, Some(&concrete));
    slab(&mut s, 20.0, 1.0, None);
    let mut game = playing(Rc::new(RefCell::new(s)), 30.0);
    // The colliders are built naming assets not written yet: they start dangling.
    run(&mut game, 1);
    let make = format!(
        "Physics.CreateMaterialAsset('{wood}', {{friction = 0.5}})\n\
         Physics.CreateMaterialAsset('{concrete}', {{friction = 0.75}})"
    );
    game.script_manager().eval(&make).expect("assets written");
    run(&mut game, 1);
    (game, wood, concrete)
}

#[test]
fn a_wallbang_prices_each_crossing_by_its_material() {
    let (game, wood, concrete) = range();
    let lua = format!(
        "local resist = {{ ['{wood}'] = 1.0, ['{concrete}'] = 0.5 }} -- per metre\n\
         local damage, seen = 100, {{}}\n\
         for _, s in ipairs(Physics.RaycastThrough(0,0,0, 0,0,1, 30)) do\n\
           table.insert(seen, (s.material and s.material:match('[^/]+$') or 'inline') .. '@' .. s.thickness)\n\
           if not s.material then break end\n\
           damage = damage * (1 - s.thickness * resist[s.material])\n\
         end\n\
         return damage, table.concat(seen, ' ')"
    );
    let got = game.script_manager().eval(&lua).expect("evaluates");
    assert_eq!(got, "25, wood.physmat@0.5 concrete.physmat@1.0 inline@1.0");
}

#[test]
fn every_cast_names_the_material_it_hit() {
    let (game, wood, _) = range();
    let eval = |lua: &str| game.script_manager().eval(lua).expect("evaluates");
    let single = "local r = {Physics.Raycast(0,0,0, 0,0,1)}; return r[13]";
    assert_eq!(eval(single), wood);
    let sphere = "local r = {Physics.SphereCast(0,0,0, 0,0,1, 0.1)}; return r[13]";
    assert_eq!(eval(sphere), wood);
    let all = "local m = {}\n\
               for _, h in ipairs(Physics.RaycastAll(0,0,0, 0,0,1, 30)) do\n\
                 m[#m + 1] = h.material and h.material:match('[^/]+$') or 'nil' end\n\
               return table.concat(m, ' ')";
    assert_eq!(eval(all), "wood.physmat concrete.physmat nil");
    let reused = "local t = {}; Physics.RaycastAllNonAlloc(t, 0,0,0, 0,0,1, 30)\n\
                  Physics.RaycastAllNonAlloc(t, 0,0,15, 0,0,1, 30)\n\
                  return t[1].material, t[2].material:match('[^/]+$')";
    assert_eq!(
        eval(reused),
        "nil, concrete.physmat",
        "a refilled entry is cleared"
    );
    let miss = "local r = {Physics.Raycast(0,0,0, 0,0,-1)}; return r[1], r[13]";
    assert_eq!(eval(miss), "false, nil");
}
