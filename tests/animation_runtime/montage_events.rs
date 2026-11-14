//! Montage events through the real tick (#1894): `OnAnimationEvent` gets a
//! third argument, `{ montage, slot, interrupted }`, for a montage's
//! `MontageBlendOut` and `MontageEnd`, and `nil` for every other event.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::script::scripting::ConsoleLogs;

use super::rig::armed_scene;

const SCRIPT: &str = r#"
local started = false
return {
  Update = function(id, dt)
    if not started then
      started = Animator.PlayMontage(id, "Slide", { blend_in = 0, blend_out = 0.1 })
    end
  end,
  OnAnimationEvent = function(id, name, info)
    if info then
      print("[ev] " .. name .. " " .. info.montage .. " " .. info.slot .. " "
        .. tostring(info.interrupted))
    else
      print("[ev] " .. name .. " nil")
    end
  end,
}"#;

#[test]
fn montage_events_say_whose_they_are_and_others_pass_nil() {
    let (scene, _, _, _) = armed_scene(Some(SCRIPT));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = crate::play::world(Rc::new(RefCell::new(scene)), 5.0, Rc::clone(&console));
    game.set_playing(true);
    crate::play::run(&mut game, 90); // 1.5 s: the 1 s montage plays out
    let log = console.borrow();
    let lines: Vec<String> = log.messages.iter().map(|(m, _)| m.clone()).collect();
    let events: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|m| m.starts_with("[ev]"))
        .collect();
    let montage: Vec<&str> = events
        .iter()
        .copied()
        .filter(|m| m.contains("Montage"))
        .collect();
    assert_eq!(
        montage,
        [
            "[ev] MontageBlendOut Slide DefaultSlot false",
            "[ev] MontageEnd Slide DefaultSlot false"
        ],
        "{lines:?}"
    );
    let others = events.iter().filter(|m| !m.contains("Montage"));
    assert!(others.clone().count() > 0, "the base clip's End: {lines:?}");
    assert!(others.into_iter().all(|m| m.ends_with(" nil")), "{lines:?}");
}
