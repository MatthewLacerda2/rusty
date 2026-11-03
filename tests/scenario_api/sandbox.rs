//! The scenario VM has the gameplay VM's deterministic stdlib (#1408): no `os` /
//! `io`, `math.random` over the world's seeded `Random`, and the project-aware
//! `require` / `dofile` (#1717).

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

/// The scenario VM's `require` and `dofile` read against the project, as in Play
/// (#1717): a game script it `dofile`s requires a module that requires another.
#[test]
fn a_scenario_requires_a_project_module_that_requires_another() {
    let dir = crate::temp::dir().join("rusty_scenario_require");
    let scripts = dir.join(native("out/workspace/assets/scripts"));
    std::fs::create_dir_all(scripts.join("lib")).unwrap();
    let write = |rel: &str, code: &str| std::fs::write(scripts.join(native(rel)), code).unwrap();
    write("lib/inner.lua", "return { value = 7 }");
    write(
        "lib/outer.lua",
        "return { value = require('lib.inner').value * 6 }",
    );
    write("uses_lib.lua", "return require('lib.outer').value");
    let code = r#"
Harness.Expect(require("lib.outer").value == 42, "require reads the project's scripts")
Harness.Expect(dofile("assets/scripts/uses_lib.lua") == 42, "dofile reads from the project root")
"#;
    let path = dir.join("scenario.lua");
    std::fs::write(&path, code).unwrap();
    let report = rusty::dev::scenario::run_isolated(&path, &dir.join("out")).unwrap();
    let json = std::fs::read_to_string(report.results_path).unwrap();
    assert!(report.passed, "a module chain loads in a scenario: {json}");
}
