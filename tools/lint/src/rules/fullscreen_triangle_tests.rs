use super::{check, judged, HOME, RULE};

#[test]
fn flags_a_pasted_triangle() {
    let src = "@vertex\nfn vs(@builtin(vertex_index) i: u32) -> vec4<f32> {\n    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));\n}\n";
    let found: Vec<_> = check("engine/shaders/a.wgsl", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(3, RULE)]);
    let spaced = "let x = f32((vid<<1u)&2u);\n";
    assert_eq!(check("engine/shaders/b.wgsl", spaced).len(), 1);
}

#[test]
fn passes_the_module_imports_and_comments() {
    let body = "let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));\n";
    assert!(check(HOME, body).is_empty());
    let user =
        "#import fullscreen\nfn vs(i: u32) -> vec4<f32> { return fullscreen::triangle(i); }\n";
    assert!(check("engine/shaders/a.wgsl", user).is_empty());
    let note = "// the old copy: f32((i << 1u) & 2u)\nlet y = i << 1u;\n";
    assert!(check("engine/shaders/a.wgsl", note).is_empty());
}

#[test]
fn a_reasoned_allow_excuses_a_copy_and_a_bare_one_is_reported() {
    let copy = "let x = f32((i << 1u) & 2u);";
    let ok = format!("// lint: allow({RULE}) test fixture, stands alone\n{copy}\n");
    assert!(judged("engine/shaders/a.wgsl", &ok).is_empty());
    let bare = format!("// lint: allow({RULE})\n{copy}\n");
    let rules: Vec<_> = judged("engine/shaders/a.wgsl", &bare)
        .iter()
        .map(|f| f.rule)
        .collect();
    assert_eq!(rules, [RULE, "allow"]);
}
