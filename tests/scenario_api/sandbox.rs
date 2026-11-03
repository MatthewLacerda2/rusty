//! The scenario VM has the gameplay VM's deterministic stdlib (#1408): no `os` /
//! `io`, `math.random` over the world's seeded `Random`, and the project-aware
//! `require` / `dofile` / `loadfile` (#1717).

use rusty::core::paths::native;

use super::run;

const DRAWS: &str = r#"
Harness.Step(1)
local draws = {}
for i = 1, 8 do draws[i] = tostring(math.random(1, 1000)) end
draws[9] = tostring(math.random())
Harness.Log("draws " .. table.concat(draws, " "))
Harness.Expect(os == nil and io == nil, "no os or io in a scenario")
"#;

/// Two runs of one scenario that draws from `math.random` write the same bytes.
#[test]
fn math_random_gives_identical_results_run_to_run() {
    let (passed, first) = run("rusty_scenario_random_a", DRAWS);
    assert!(passed, "scenario expectations pass: {first}");
    let (_, second) = run("rusty_scenario_random_b", DRAWS);
    assert_eq!(
        first, second,
        "results.json is a pure function of the scenario"
    );
    assert!(first.contains("draws "), "the draws were logged: {first}");
}

/// The scenario VM's `require`, `dofile` and `loadfile` read against the project,
/// as in Play (#1717): a game script it `dofile`s requires a module that requires
/// another, and a file outside the project loads but warns once in the console.
#[test]
fn a_scenario_requires_a_project_module_and_warns_once_outside_it() {
    let name = "rusty_scenario_require";
    let dir = crate::temp::dir().join(name);
    let scripts = dir.join(native("out/workspace/assets/scripts"));
    std::fs::create_dir_all(scripts.join("lib")).unwrap();
    let write = |at: &std::path::Path, rel: &str, code: &str| {
        std::fs::write(at.join(native(rel)), code).unwrap()
    };
    write(&scripts, "lib/inner.lua", "return { value = 7 }");
    write(
        &scripts,
        "lib/outer.lua",
        "return { value = require('lib.inner').value * 6 }",
    );
    write(
        &scripts,
        "uses_lib.lua",
        "return require('lib.outer').value",
    );
    write(&dir, "outside_do.lua", "return 1");
    write(&dir, "outside_load.lua", "return 2");
    let code = r#"
Harness.Expect(require("lib.outer").value == 42, "require reads the project's scripts")
Harness.Expect(dofile("assets/scripts/uses_lib.lua") == 42, "dofile reads from the project root")
Harness.Expect(loadfile("assets/scripts/uses_lib.lua")() == 42, "so does loadfile")
Harness.Expect(dofile("../../outside_do.lua") + dofile("../../outside_do.lua") == 2, "outside loads")
Harness.Expect(loadfile("../../outside_load.lua")() == 2, "loadfile outside loads")
"#;
    let (passed, json) = run(name, code);
    assert!(passed, "a module chain loads in a scenario: {json}");
    let log = std::fs::read_to_string(dir.join(native("out/console.log"))).unwrap();
    let warned = |file: &str| {
        log.lines()
            .filter(|l| l.contains(file) && l.contains("outside the project"))
            .count()
    };
    assert_eq!(warned("outside_do.lua"), 1, "dofile warns once: {log}");
    assert_eq!(warned("outside_load.lua"), 1, "loadfile warns: {log}");
}
