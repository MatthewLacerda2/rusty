//! The `AIPerception` namespace (#1900): `ReportNoise`, `GetKnown`, `Forget` and
//! the settings through the shared ops.

use glam::Vec3;

use super::{arena, eval, play};

#[test]
fn a_noise_is_counted_heard_and_dispatched_every_time() {
    let (scene, bot, player) = arena(Vec3::new(0.0, 1.0, -20.0));
    let mut game = play(scene, 1);
    let line = format!(
        "local far = AIPerception.ReportNoise(0, 1, -40, 1, {{ instigator = {player} }}) \
         local near = AIPerception.ReportNoise(0, 1, -20, 1, {{ instigator = {player}, tag = 'gunshot' }}) \
         local nobody = AIPerception.ReportNoise(0, 1, -5, 0.5) \
         local k = AIPerception.Get({bot}, {player}) \
         return far, near, nobody, k.sense, k.tag, #AIPerception.GetKnown({bot})"
    );
    assert_eq!(eval(&game, &line), "0, 1, 1, hearing, gunshot, 1");
    crate::play::run(&mut game, 1);
    let line = "return #LOG, LOG[1].tag, LOG[2].target == nil";
    assert_eq!(eval(&game, line), "2, gunshot, true");
    let bad = "return pcall(AIPerception.ReportNoise, 0, 0, 0, -1)";
    assert!(
        eval(&game, bad).starts_with("false"),
        "a negative loudness errors"
    );
    let line = format!("AIPerception.Forget({bot}) return #AIPerception.GetKnown({bot})");
    assert_eq!(eval(&game, &line), "0");
}

#[test]
fn the_settings_round_trip_through_the_shared_ops() {
    let (scene, bot, _) = arena(Vec3::new(0.0, 1.0, -20.0));
    let game = play(scene, 1);
    let line = format!(
        "AIPerception.SetSightRange({bot}, 40) AIPerception.SetSightAngle({bot}, 999) \
         AIPerception.SetHearingRange({bot}, 0) AIPerception.SetSightMask({bot}, 8) \
         return AIPerception.GetSightRange({bot}), AIPerception.GetLoseSightRange({bot}), \
           AIPerception.GetSightAngle({bot}), AIPerception.GetHearingRange({bot}), \
           AIPerception.GetSightMask({bot}), AIPerception.GetEyeHeight(9999)"
    );
    assert_eq!(eval(&game, &line), "40, 40, 180, 0, 8, 0");
    let line = "AIPerception.SetSightRange(9999, 3) \
                return pcall(AIPerception.SetSightRange, nil, 3)";
    let answer = eval(&game, line);
    assert!(
        answer.starts_with("false"),
        "a dead id is a no-op, nil errors"
    );
    // A value the component would not keep is refused, not floored: a max age
    // of 0 means forever.
    for call in [
        "SetSightMaxAge({bot}, -1)",
        "SetHearingRange({bot}, -3)",
        "SetEyeHeight({bot}, 1/0)",
    ] {
        let line = format!("return pcall(function() AIPerception.{call} end)");
        let line = line.replace("{bot}", &bot.to_string());
        assert!(eval(&game, &line).starts_with("false"), "{call} errors");
    }
    let line =
        format!("return AIPerception.GetSightMaxAge({bot}), AIPerception.GetHearingRange({bot})");
    assert_eq!(eval(&game, &line), "5, 0", "unchanged");
}
