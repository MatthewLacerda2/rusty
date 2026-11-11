use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_uncounted_inserts_removes_and_retypes() {
    let at = "src/physics/joints/mod.rs";
    assert_eq!(found(at, "let h = self.bodies.insert(b);\n"), [1]);
    assert_eq!(found(at, "bodies.remove(h, a, b, c, d, e, true);\n"), [1]);
    assert_eq!(found(at, "body.set_body_type(class, true);\n"), [1]);
    let wrapped = "self\n    .bodies\n    .remove(handle);\n";
    assert_eq!(found(at, wrapped), [3]);
}

#[test]
fn passes_counted_calls_reads_other_sets_tests_and_other_modules() {
    let at = "src/physics/assemble.rs";
    assert!(found(at, "let h = self.insert_body(b);\nself.remove_body(h);\n").is_empty());
    assert!(found(at, "let b = self.bodies.get(h)?;\n").is_empty());
    assert!(found(at, "self.id_to_body.remove(&owner);\n").is_empty());
    assert!(found(
        at,
        "self.colliders.insert_with_parent(c, h, &mut self.bodies);\n"
    )
    .is_empty());
    assert!(found(at, "// self.bodies.insert(b)\n").is_empty());
    let raw = "self.bodies.insert(b);\n";
    assert!(found("src/components/mesh/skeleton.rs", raw).is_empty());
    assert!(found("src/physics/live_tests.rs", raw).is_empty());
    let gated = "#[cfg(test)]\nfn t(b: &mut B) { b.set_body_type(t, true); }\n";
    assert!(found(at, gated).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_a_helper() {
    let src = "// lint: allow(body-table) the one insert: the revision moved above\n\
               self.bodies.insert(body)\n";
    let found = check("src/physics/character/bodies.rs", src, &[false, false]);
    assert_eq!(found.len(), 1);
    assert!(allowed(src, found[0].line, RULE));
}
