//! The bundled Player controller (#748): Space jumps — grounded only, on the rising
//! edge, peaking ≈1.1 m — and left mouse (`Mouse0`) shoots once per press.

use rusty::dev::botplayer::attach_player_bot;
use rusty::dev::console::evaluate_line;
use rusty::dev::harness::Harness;

/// Evaluate `line` in the harness world's gameplay VM.
fn eval(h: &Harness, line: &str) -> String {
    let world = h.world.borrow();
    evaluate_line(world.script_manager(), world.console(), line).expect("line evaluates")
}

/// The Player's height, read through the same API the scripts use.
fn player_y(h: &Harness) -> f32 {
    let y = "local _, y = Transform.GetPosition(Scene.FindEntityByName('Player')); return y";
    eval(h, y).parse().expect("a number")
}

fn press(h: &Harness, key: &str) {
    h.world.borrow().input().borrow_mut().press(key);
}

fn release(h: &Harness, key: &str) {
    h.world.borrow().input().borrow_mut().release(key);
}

/// A harness with no enemy brain, stepped until the Player rests on the floor.
fn settled(name: &str) -> (Harness, f32) {
    let h = Harness::new(crate::temp::dir().join(name), "");
    h.step(60);
    let ground = player_y(&h);
    h.step(10);
    assert!(
        (player_y(&h) - ground).abs() < 1e-3,
        "the Player starts at rest"
    );
    (h, ground)
}

/// Step `ticks` frames, returning the highest the Player rose above `ground`.
fn apex_over(h: &Harness, ground: f32, ticks: u32) -> f32 {
    let mut apex = f32::MIN;
    for _ in 0..ticks {
        h.step(1);
        apex = apex.max(player_y(h) - ground);
    }
    apex
}

#[test]
fn space_jumps_about_a_metre_and_lands() {
    let (h, ground) = settled("rusty_player_jump");
    press(&h, "SPACE");
    let apex = apex_over(&h, ground, 60);
    assert!((1.0..=1.2).contains(&apex), "apex ≈1.1 m, got {apex}");
    assert!((player_y(&h) - ground).abs() < 0.05, "back on the floor");

    // Space is still held: landing must not bounce into another jump.
    let rest = apex_over(&h, ground, 30);
    assert!(rest < 0.05, "holding Space does not re-jump, rose {rest}");
}

#[test]
fn no_double_jump_mid_air() {
    let (h, ground) = settled("rusty_player_double_jump");
    press(&h, "SPACE");
    let rising = apex_over(&h, ground, 10);
    release(&h, "SPACE");
    h.step(1);
    press(&h, "SPACE"); // a fresh rising edge, in the air
    let apex = rising.max(apex_over(&h, ground, 60));
    assert!(apex <= 1.2, "a mid-air press adds no height, apex {apex}");
    assert!((player_y(&h) - ground).abs() < 0.05, "back on the floor");
}

/// Drives the bundled controller, counting the casts it fires into `SHOTS`. The API
/// is registered afresh each tick, so the wrap is laid inside `Update`.
const SHOT_PROBE: &str = r#"
local Controller = dofile("project/assets/scripts/player_controller.lua")
local Probe = {}
SHOTS = 0
function Probe.Start(id) Controller.Start(id) end
function Probe.Update(id, dt)
    local cast = Physics.Raycast
    Physics.Raycast = function(...) SHOTS = SHOTS + 1; return cast(...) end
    Controller.drive(Controller, id, dt)
end
return Probe
"#;

#[test]
fn left_mouse_shoots_once_per_press_and_space_does_not() {
    let dir = crate::temp::dir().join("rusty_player_shoot");
    std::fs::create_dir_all(&dir).unwrap();
    let probe = dir.join("shot_probe.lua");
    std::fs::write(&probe, SHOT_PROBE).unwrap();
    let h = Harness::new(&dir, "");
    let path = probe.to_str().unwrap();
    assert!(attach_player_bot(
        &mut h.world.borrow().scene().borrow_mut(),
        path
    ));
    h.step(60);
    let shots = |h: &Harness| eval(h, "return SHOTS");
    assert_eq!(shots(&h), "0", "no shot without a press");

    press(&h, "SPACE");
    h.step(5);
    release(&h, "SPACE");
    assert_eq!(shots(&h), "0", "Space jumps, it does not shoot");

    press(&h, "Mouse0");
    h.step(5);
    assert_eq!(
        shots(&h),
        "1",
        "one cast per press, however long it is held"
    );
    release(&h, "Mouse0");
    h.step(1);
    press(&h, "Mouse0");
    h.step(1);
    assert_eq!(shots(&h), "2", "the next press fires again");
}
