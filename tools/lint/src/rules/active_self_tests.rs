use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_a_system_reading_the_own_flag() {
    let src = "fn draw(w: &World, id: u32) {\n    if !w.is_active_self(id) { return; }\n}\n";
    assert_eq!(found("src/render/draw/mod.rs", src), [2]);
}

#[test]
fn passes_the_hierarchy_query_the_ecs_tests_and_other_roots() {
    let ask = "if !w.is_active_in_hierarchy(id) { return; }\n";
    assert!(found("src/render/draw/mod.rs", ask).is_empty());
    let own = "w.is_active_self(id)\n";
    assert!(found("src/ecs/core.rs", own).is_empty());
    assert!(found("src/scene/a_tests.rs", own).is_empty());
    assert!(found("tests/a.rs", own).is_empty());
    assert!(found("src/a.rs", "// is_active_self(id)\nw.was_active_self(id)\n").is_empty());
    let gated = "#[cfg(test)]\nmod tests {\n    fn t() { w.is_active_self(1); }\n}\n";
    assert!(found("src/a.rs", gated).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(active-self) the checkbox shows the own flag\nw.is_active_self(id)\n";
    let found = check("src/a.rs", src, &test_lines("src/a.rs", src, false));
    assert_eq!(found.len(), 1);
    assert!(allowed(src, found[0].line, RULE));
}
