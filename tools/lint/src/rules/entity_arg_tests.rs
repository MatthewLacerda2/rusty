use super::{check, RULE};
use crate::rules::{allowed, test_code::test_lines};

fn found(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &test_lines(path, src, false))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_a_callback_taking_a_bare_id() {
    let src = "let f = scope.create_function(move |_, id: u32| Ok(id));\n\
               let g = scope.create_function(|lua, (id, x): (u32, f32)| Ok(x));\n\
               let h = move |_: &Lua, (root_id, n): (Option<u32>, String)| Ok(n);\n\
               let k = scope.create_function(|_, (x, parent): (f32, Option<u32>)| Ok(x));\n";
    assert_eq!(found("src/api/scene.rs", src), [1, 2, 3, 4]);
}

#[test]
fn passes_entity_arg_other_counts_helpers_and_other_roots() {
    let ok = "scope.create_function(move |_, id: EntityArg| Ok(id.read()));\n\
              scope.create_function(|_, (id, n): (EntityArg, u32)| Ok(n));\n\
              scope.create_function(|_, (w, h): (u32, u32)| Ok(w));\n\
              scope.create_function(|_, mask: Option<u32>| Ok(mask));\n\
              let edit = move |id: u32, f: &dyn Fn(&mut R)| f(id);\n\
              let s = move |lua: &Lua, id: u32, t: Value, d: f64| Ok(());\n\
              let m = a | b | c;\n";
    assert!(found("src/api/scene.rs", ok).is_empty());
    let bare = "scope.create_function(move |_, id: u32| Ok(id));\n";
    assert!(found("src/scripting/a.rs", bare).is_empty());
    assert!(found("src/api/a_tests.rs", bare).is_empty());
    assert!(found("src/api/a.rs", &format!("// {bare}")).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(entity-arg) a raw slot index, not an entity\n\
               scope.create_function(|_, id: u32| Ok(id));\n";
    let found = check("src/api/a.rs", src, &test_lines("src/api/a.rs", src, false));
    assert_eq!(found.len(), 1);
    assert!(allowed(src, found[0].line, RULE));
}
