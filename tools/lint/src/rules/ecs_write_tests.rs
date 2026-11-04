use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_raw_mutable_borrows_and_reshapes_in_the_ecs() {
    let at = "src/ecs/core.rs";
    assert_eq!(
        found(at, "let c = self.inner.get::<&mut Core>(h).ok();\n"),
        [1]
    );
    let wrapped = "let mut q = self\n    .inner\n    .query_one::<(&mut A, Option<&mut B>)>(h)\n";
    assert_eq!(found(at, wrapped), [3]);
    assert_eq!(
        found(at, "for (_, c) in self.inner.query_mut::<&mut Core>() {}\n"),
        [1]
    );
    assert_eq!(found(at, "let _ = self.inner.insert_one(h, v);\n"), [1]);
    assert_eq!(found(at, "let _ = self.inner.remove_one::<T>(h);\n"), [1]);
    assert_eq!(found(at, "let h = self.inner.spawn(b.build());\n"), [1]);
    assert_eq!(
        found(at, "self.inner.despawn(h).ok();\nself.inner.clear();\n"),
        [1, 2]
    );
}

#[test]
fn passes_reads_other_receivers_tests_and_other_modules() {
    let at = "src/ecs/core.rs";
    assert!(found(at, "let c = self.inner.get::<&Core>(h).ok();\n").is_empty());
    assert!(found(at, "self.inner.query::<&Core>().iter();\n").is_empty());
    assert!(found(at, "let v = self.order.get_mut(3);\nself.despawn(id);\n").is_empty());
    assert!(found(at, "self.order.clear();\nself.winner.remove(&k);\n").is_empty());
    assert!(found(at, "// self.inner.get::<&mut Core>(h)\n").is_empty());
    let raw = "self.inner.get::<&mut Core>(h);\n";
    assert!(found("src/render/a.rs", raw).is_empty());
    assert!(found("src/ecs/changes/tests.rs", raw).is_empty());
    let gated = "#[cfg(test)]\nfn t(w: &mut W) { w.inner.spawn(()); }\n";
    assert!(found(at, gated).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_a_funnel_line() {
    let src = "// lint: allow(ecs-write) the funnel: touch stamped it above\n\
               self.inner.get::<&mut T>(h).ok()\n";
    let found = check("src/ecs/world.rs", src, &[false, false]);
    assert_eq!(found.len(), 1);
    assert!(allowed(src, found[0].line, RULE));
}
