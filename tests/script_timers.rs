//! Script timers and coroutines end to end (#444): through the real play schedule
//! (the harness steps `GameWorld::tick`), an `Invoke`, an `InvokeRepeating` and a
//! `WaitForSeconds` coroutine fire on fixed ticks, and two runs print the same
//! lines. Gated on `dev`, like the harness itself.

use rusty::dev::harness::Harness;

const SCRIPT: &str = r#"
local Brain = { t = 0 }
local function mark(tag) print(string.format("[timer] %s %d", tag, Brain.t)) end
function Brain.Start(id)
    Timer.Invoke(id, function() mark("once") end, 0.05)
    Timer.InvokeRepeating(id, function() mark("rep") end, 0.1, 0.1)
    Timer.StartCoroutine(id, function()
        for _ = 1, 2 do
            coroutine.yield(Timer.WaitForSeconds(0.25))
            mark("co")
        end
    end)
end
function Brain.Update(id) Brain.t = Brain.t + 1 end
return Brain
"#;

fn timer_lines(dir: &str, frames: u32) -> Vec<String> {
    let out = crate::temp::dir().join(dir);
    std::fs::create_dir_all(&out).expect("temp dir");
    let script = out.join("timer_brain.lua");
    std::fs::write(&script, SCRIPT).expect("write script");
    let h = Harness::new(&out, &script.to_string_lossy());
    h.step(frames);
    let console = h.console.borrow();
    console
        .messages
        .iter()
        .map(|m| m.0.clone())
        .filter(|m| m.contains("[timer]") || m.contains("Lua Error"))
        .collect()
}

#[test]
fn timers_fire_on_fixed_ticks_and_replay_identically() {
    let a = timer_lines("rusty_script_timers_a", 40);
    let count = |tag: &str| a.iter().filter(|l| l.contains(tag)).count();
    assert_eq!(count("once"), 1, "{a:?}");
    assert_eq!(
        count("rep"),
        6,
        "0.1 s repeats across 39 counted steps: {a:?}"
    );
    assert_eq!(count("co"), 2, "{a:?}");
    assert!(!a.iter().any(|l| l.contains("Lua Error")), "{a:?}");
    assert_eq!(a, timer_lines("rusty_script_timers_b", 40));
}
