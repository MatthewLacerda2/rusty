//! Lua-side determinism (#443): two headless harness runs of a script that draws
//! from `math.random` and `Random.*` every tick see the same numbers, and the
//! gameplay VM has no wall clock. Gated on `dev`, like the harness itself.

use rusty::dev::harness::Harness;

const SCRIPT: &str = r#"
local Brain = {}
function Brain.Update(entity_id, dt)
    local x, y, z = Random.InsideUnitSphere()
    print(string.format("[rng] %d %.17g %d %.9g %.9g %.9g %s",
        math.random(1, 1000), math.random(), Random.Range(0, 50), x, y, z, tostring(os)))
end
return Brain
"#;

/// Run `frames` ticks with the RNG script as the enemy brain; return its prints.
fn rng_lines(dir: &str, frames: u32) -> Vec<String> {
    let out = std::env::temp_dir().join(dir);
    std::fs::create_dir_all(&out).expect("temp dir");
    let script = out.join("rng_brain.lua");
    std::fs::write(&script, SCRIPT).expect("write script");
    let h = Harness::new(&out, &script.to_string_lossy());
    h.step(frames);
    let console = h.console.borrow();
    console
        .messages
        .iter()
        .map(|m| m.0.clone())
        .filter(|m| m.contains("[rng]"))
        .collect()
}

#[test]
fn two_harness_runs_draw_identical_random_streams() {
    let a = rng_lines("rusty_lua_determinism_a", 30);
    assert_eq!(a.len(), 30, "the brain must print once per tick: {a:?}");
    assert!(
        a.iter().all(|l| l.ends_with(" nil")),
        "os must be nil: {a:?}"
    );
    assert_ne!(a[0], a[1], "the stream must advance tick to tick");
    assert_eq!(a, rng_lines("rusty_lua_determinism_b", 30));
}
