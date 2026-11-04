use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_raw_mutable_borrows_of_bodies_and_colliders() {
    let at = "src/physics/live.rs";
    assert_eq!(
        found(at, "if let Some(c) = self.colliders.get_mut(h) {}\n"),
        [1]
    );
    assert_eq!(found(at, "for (_, b) in bodies.iter_mut() {}\n"), [1]);
    assert_eq!(found(at, "let body = &mut self.bodies[handle];\n"), [1]);
    assert_eq!(found(at, "self.colliders[h].set_sensor(true);\n"), [1]);
    assert_eq!(found(at, "world.bodies[h] = body;\n"), [1]);
    let wrapped = "let c = self\n    .colliders\n    .get_mut(handle);\n";
    assert_eq!(found(at, wrapped), [3]);
}

#[test]
fn passes_reads_other_sets_tests_and_other_modules() {
    let at = "src/physics/live.rs";
    assert!(found(at, "let c = self.colliders.get(h)?;\n").is_empty());
    assert!(found(at, "let b = &self.bodies[h];\nlet m = bodies[h].mass();\n").is_empty());
    assert!(found(at, "if bodies[h] == other {}\n").is_empty());
    assert!(found(at, "for (o, ids) in plan.iter_mut() {}\n").is_empty());
    assert!(found(at, "let h = id_to_colliders.get_mut(&id);\n").is_empty());
    assert!(found(at, "// self.colliders.get_mut(h)\n").is_empty());
    let raw = "self.colliders.get_mut(h);\n";
    assert!(found("src/render/a.rs", raw).is_empty());
    assert!(found("src/physics/live_tests.rs", raw).is_empty());
    let gated = "#[cfg(test)]\nfn t(w: &mut W) { w.bodies.get_mut(h); }\n";
    assert!(found(at, gated).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_a_helper() {
    let src = "// lint: allow(physics-borrow) the helper: `differs` compared first\n\
               colliders.get_mut(handle)\n";
    let found = check("src/physics/world.rs", src, &[false, false]);
    assert_eq!(found.len(), 1);
    assert!(allowed(src, found[0].line, RULE));
}
