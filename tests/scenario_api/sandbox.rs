//! The scenario VM has the gameplay VM's deterministic stdlib (#1408): no `os` /
//! `io`, and `math.random` over the world's seeded `Random`.

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
