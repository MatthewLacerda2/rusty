use super::{check, RULE};

fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src).iter().map(|f| f.line).collect()
}

#[test]
fn flags_an_edit_of_a_mesh_buffer_outside_the_accessors() {
    let src = "fn f(m: &mut M) {\n    m.vertices.edit()[0].position = p;\n}\n";
    assert_eq!(lines("crates/scene/src/a.rs", src), [2]);
    assert!(check("crates/scene/src/a.rs", src)
        .iter()
        .all(|f| f.rule == RULE));
    let indices = "mesh.indices.edit().swap(0, 1);\n";
    assert_eq!(lines("crates/lighting/src/a_tests.rs", indices), [1]);
    assert_eq!(lines("tests/gpu/a.rs", indices), [1]);
}

#[test]
fn passes_the_accessors_the_importer_and_strings() {
    let accessor = "mesh.vertices_mut()[0].position = p;\n";
    assert!(lines("crates/scene/src/a.rs", accessor).is_empty());
    let edit = "self.vertices.edit()\n";
    assert!(lines("crates/components/src/mesh/geometry.rs", edit).is_empty());
    assert!(lines("crates/asset/src/obj_import.rs", edit).is_empty());
    assert!(lines("crates/scene/src/a.rs", "log(\"m.vertices.edit()\");\n").is_empty());
    assert!(lines("tools/lint/src/a.rs", edit).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(mesh-buffer-write) a fixture no renderer draws\nm.indices.edit();\n";
    let found = check("crates/scene/src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
