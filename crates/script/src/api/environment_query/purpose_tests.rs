//! A test entry whose fields contradict its `purpose` is a Lua error naming the
//! field (#1892, the review of PR #1921), never a field silently ignored.

use super::tests::{error, vm, RING};

#[test]
fn a_purpose_a_field_contradicts_is_an_error_naming_the_field() {
    let (lua, _) = vm();
    let cases = [
        (
            "{ distance = {}, purpose = 'score', max = 3 }",
            "tests[3].max: purpose 'score' does not filter",
        ),
        (
            "{ distance = {}, purpose = 'filter', min = 1, score = {} }",
            "tests[3].score: purpose 'filter' does not score",
        ),
        (
            "{ path = 'exists', min = 1 }",
            "tests[3].min: a yes/no test filters on its answer",
        ),
        (
            "{ trace = { visible = false }, max = 0 }",
            "tests[3].max: a yes/no test filters on its answer",
        ),
    ];
    for (entry, want) in cases {
        lua.load(RING).exec().unwrap();
        let code = format!(
            "spec.tests[3] = {entry}; EnvironmentQuery.Run(bot, spec, {{ target = player }})"
        );
        let got = error(&lua, &code);
        assert!(got.contains(want), "{entry}: {got}");
    }
    // Consistent ones still run.
    lua.load(RING).exec().unwrap();
    let ok = "spec.tests[3] = { distance = {}, purpose = 'filter_score', max = 9, score = {} }; \
              EnvironmentQuery.Run(bot, spec, { target = player })";
    lua.load(ok).exec().unwrap();
}
