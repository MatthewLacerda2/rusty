//! Tweens end to end (#424): through the real play schedule (the harness steps
//! `GameWorld::tick`), a UI fade, an overshooting slide and a yoyo light pulse
//! write the same values on the same ticks in two runs, and land where they
//! should. Gated on `dev`, like the harness itself.

use rusty::dev::harness::Harness;

const SCRIPT: &str = r#"
local Brain = {}
function Brain.Start(_)
    local id = Scene.CreateEntity("Pivot")
    Brain.pivot = id
    local hud = Scene.CreateEntity("Hud")
    Scene.AddComponent(hud, "CanvasGroup")
    Scene.AddComponent(hud, "Image")
    Brain.hud = hud
    Tween.To(hud, "CanvasGroup.alpha", 1, 0.25, { from = 0, ease = "quad_out" })
    Tween.To(hud, "Image.color", {1, 0, 0, 1}, 0.2, { delay = 0.1, ease = "sine_in_out" })
    Tween.To(id, "Transform.position", {3, 0, 0}, 0.3, { ease = "back_out",
        on_complete = function() print("[tween] landed") end })
    Tween.To(id, "Transform.scale", {2, 2, 2}, 0.1, { ease = "elastic_out", loops = 4, yoyo = true })
end
function Brain.Update(_)
    local x = Transform.GetPosition(Brain.pivot)
    local s = Transform.GetScale(Brain.pivot)
    local _, g = Image.GetColor(Brain.hud)
    print(string.format("[tween] %.9g %.9g %.9g %.9g", x, s, CanvasGroup.GetAlpha(Brain.hud), g))
end
return Brain
"#;

fn tween_lines(dir: &str, frames: u32) -> Vec<String> {
    let out = crate::temp::dir().join(dir);
    std::fs::create_dir_all(&out).expect("temp dir");
    let script = out.join("tween_brain.lua");
    std::fs::write(&script, SCRIPT).expect("write script");
    let h = Harness::new(&out, &script.to_string_lossy());
    h.step(frames);
    let console = h.console.borrow();
    console
        .messages
        .iter()
        .map(|m| m.0.clone())
        .filter(|m| m.contains("[tween]") || m.contains("Lua Error"))
        .collect()
}

#[test]
fn tweens_replay_identically_and_land_on_their_targets() {
    let a = tween_lines("rusty_tweens_a", 40);
    assert!(!a.iter().any(|l| l.contains("Lua Error")), "{a:?}");
    assert_eq!(
        a.iter().filter(|l| l.contains("landed")).count(),
        1,
        "{a:?}"
    );
    let last = a.last().expect("the brain prints every tick");
    assert_eq!(last, "[tween] 3 1 1 0", "{a:?}");
    let overshot = a.iter().any(|l| {
        let x: f64 = l
            .split(' ')
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        x > 3.0
    });
    assert!(overshot, "back_out passes its target on the way: {a:?}");
    assert_eq!(a, tween_lines("rusty_tweens_b", 40));
}
