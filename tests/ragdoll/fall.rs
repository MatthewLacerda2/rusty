//! A ragdoll in the real tick: it collapses onto the floor and comes to rest, a
//! shot's impulse throws it the way the shot went, and two runs match bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

use super::{hitbox_at, ragdolled, rig};

/// On the second `Update`, shoot the head from -X: ragdoll the root the ray
/// names and push the bone it hit at the hit point with `{J}` N·s along the ray.
const SHOOTER: &str = "local n = 0\n\
return { Update = function(id)\n\
  n = n + 1\n\
  if n ~= 2 then return end\n\
  local mask = 1 << Layers.NameToIndex('Hitbox')\n\
  local hit, _, _, px, py, pz, _, _, _, bone, _, root = Physics.Raycast(-3, 1.72, 0, 1, 0, 0, nil, mask)\n\
  assert(hit and bone, 'the shot hits a bone')\n\
  Ragdoll.Enable(root)\n\
  Physics.AddImpulseAtPosition(bone, {J}, 0, 0, px, py, pz)\n\
end }";

/// Play the ragdolled rig for `ticks` with a shot of `impulse` N·s at the head on
/// the second tick; return the scene and the rig.
fn shot(impulse: f32, ticks: usize, tag: &str) -> (Rc<RefCell<Scene>>, u32) {
    let (mut scene, rig) = ragdolled();
    let path = crate::temp::dir().join(format!("rusty_466_{tag}.lua"));
    std::fs::write(&path, SHOOTER.replace("{J}", &impulse.to_string())).unwrap();
    *scene.world.scripts_mut(rig).unwrap() = vec![ScriptComponent {
        path: path.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = GameWorld::new(
        scene.clone(),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    gw.set_playing(true);
    for _ in 0..ticks {
        gw.tick(1.0 / 60.0);
    }
    (scene, rig)
}

/// Every ragdoll bone's hitbox centre.
fn parts(scene: &Scene, rig: u32) -> Vec<Vec3> {
    let bones = scene.ragdoll_bones(rig);
    bones.into_iter().map(|b| hitbox_at(scene, b)).collect()
}

#[test]
fn a_ragdoll_collapses_onto_the_floor_and_comes_to_rest() {
    let (scene, rig) = shot(0.0, 240, "rest");
    let s = scene.borrow();
    assert!(s.ragdoll_enabled(rig));
    let hips = hitbox_at(&s, rig::bone(&s, rig, "Hips"));
    assert!(hips.y < 0.5, "it fell: hips at {hips}");
    for p in parts(&s, rig) {
        assert!(p.y > 0.0, "nothing sank through the floor: {p}");
        assert!(
            p.x.abs() < 1.5 && p.z.abs() < 1.5,
            "it stayed in place: {p}"
        );
    }
    for b in s.ragdoll_bones(rig) {
        let v = s.world.rigidbody(b).unwrap().velocity;
        assert!(v.length() < 0.3, "it came to rest: {v}");
    }
}

#[test]
fn a_shot_at_the_head_throws_the_body_the_way_the_shot_went() {
    let still = shot(0.0, 60, "still");
    let hit = shot(150.0, 60, "hit");
    let head = |(s, rig): &(Rc<RefCell<Scene>>, u32)| {
        let s = s.borrow();
        hitbox_at(&s, rig::bone(&s, *rig, "Head"))
    };
    let hips = |(s, rig): &(Rc<RefCell<Scene>>, u32)| {
        let s = s.borrow();
        hitbox_at(&s, rig::bone(&s, *rig, "Hips"))
    };
    let (dhead, dhips) = (head(&hit) - head(&still), hips(&hit) - hips(&still));
    assert!(dhead.x > 0.3, "the head flew along +X: {dhead}");
    assert!(
        dhead.x > dhips.x,
        "and tipped the body over: {dhead} vs {dhips}"
    );
    // A collapsing ragdoll twists a little as it falls: rapier 0.36 drifts the
    // head ~0.28 m sideways against a ~1.4 m throw (~11°), and the exact figure
    // differs per CPU architecture (its SIMD solver rounds differently on x86
    // and ARM). Under 30% (~17°) still rules out a throw to the side.
    assert!(dhead.z.abs() < 0.3 * dhead.x, "nowhere else: {dhead}");
}

#[test]
fn the_same_shot_falls_the_same_way_twice() {
    let bits = |tag: &str| {
        let (scene, rig) = shot(150.0, 120, tag);
        let s = scene.borrow();
        parts(&s, rig)
            .into_iter()
            .flat_map(|p| p.to_array().map(f32::to_bits))
            .collect::<Vec<u32>>()
    };
    assert_eq!(bits("det_a"), bits("det_b"));
}
