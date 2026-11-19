use super::is_tool_named;

#[test]
fn flags_test_files_named_for_the_tool() {
    assert!(is_tool_named(
        "crates/asset/src/uv/tests/survivors.rs",
        false
    ));
    assert!(is_tool_named(
        "crates/dev/src/bake/survivor_tests.rs",
        false
    ));
    assert!(is_tool_named("tests/nav/mutants.rs", false));
    assert!(
        is_tool_named("crates/a/src/mutation_pins.rs", true),
        "a test-only module"
    );
}

#[test]
fn passes_behaviour_names_and_non_test_code() {
    assert!(!is_tool_named("crates/asset/src/uv/tests/growth.rs", false));
    assert!(
        !is_tool_named("crates/ai/src/query/survivors.rs", false),
        "engine code"
    );
    assert!(!is_tool_named("tests/nav/agents_values.rs", false));
    assert!(!is_tool_named("crates/a/src/survivors.wgsl", true));
}
