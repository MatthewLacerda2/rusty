use super::{brackets, item_end, strip_comment, Open};

fn bracket_delta(code: &str) -> i32 {
    brackets(code, &mut Open::None).1
}

/// The last line (1-based) of the item at line `from` (1-based) of `src`.
fn end(src: &str, from: usize) -> usize {
    let lines: Vec<&str> = src.lines().collect();
    item_end(&lines, from - 1) + 1
}

#[test]
fn a_gated_fn_ends_at_its_body_whatever_its_signature_wraps() {
    let wrapped_where =
        "#[cfg(test)]\nfn f<T>(t: T)\nwhere\n    T: Clone,\n{\n    t;\n}\nfn g() {}";
    assert_eq!(end(wrapped_where, 1), 7);
    let wrapped_return =
        "#[cfg(test)]\nfn f() -> Result<\n    A,\n    B,\n> {\n    x\n}\nfn g() {}";
    assert_eq!(end(wrapped_return, 1), 7);
    let wrapped_generics = "#[cfg(test)]\nfn f<\n    T: X,\n>(t: T) {\n    t;\n}\nfn g() {}";
    assert_eq!(end(wrapped_generics, 1), 6);
    let wrapped_impl = "#[cfg(test)]\npub(crate) impl<T> X for Y<T>\nwhere\n    T: A,\n{\n}\n";
    assert_eq!(end(wrapped_impl, 1), 6);
}

#[test]
fn items_end_on_their_semicolon_or_brace() {
    assert_eq!(end("#[cfg(test)]\nuse crate::x::Helper;\nfn f() {}", 1), 2);
    assert_eq!(
        end(
            "#[cfg(test)] use crate::x::{\n    A,\n    B,\n};\nfn f() {}",
            1
        ),
        4
    );
    assert_eq!(
        end(
            "#[cfg(test)]\n#[allow(dead_code)]\nmod tests {\n    fn t() {}\n}\nx",
            1
        ),
        5
    );
}

#[test]
fn a_field_variant_or_arm_ends_on_its_comma() {
    assert_eq!(
        end(
            "struct S {\n    #[cfg(test)]\n    a: Vec<(\n        u8,\n    )>,\n    b: u8,\n}",
            2
        ),
        5
    );
    assert_eq!(
        end(
            "match x {\n    #[cfg(test)]\n    A => 1,\n    B => 2,\n}",
            2
        ),
        3
    );
}

#[test]
fn a_last_field_without_a_comma_ends_before_its_parent_closes() {
    assert_eq!(
        end("struct S {\n    #[cfg(test)]\n    a: u8\n}\nfn f() {}", 2),
        3
    );
    assert_eq!(end("enum E {\n    A,\n    #[cfg(test)]\n    B\n}", 3), 4);
}

#[test]
fn brackets_and_slashes_inside_literals_do_not_count() {
    assert_eq!(bracket_delta(r#"json!({ "uri": "rusty://nope.md" })"#), 0);
    assert_eq!(bracket_delta(r#"format!("fn {name}(")"#), 0);
    assert_eq!(bracket_delta("Some(')')"), 0);
    assert_eq!(bracket_delta(r#"let s = "]";"#), 0);
    assert_eq!(bracket_delta(r#"let s = "a\"(";"#), 0);
    assert_eq!(bracket_delta(r##"let s = r#"("#;"##), 0);
    assert_eq!(bracket_delta("fn f<'a>(x: &'a u8) {"), 1);
    assert_eq!(bracket_delta(r"Some('\'')"), 0);
    assert_eq!(
        strip_comment(r#"let u = "rusty://x"; // (note"#),
        r#"let u = "rusty://x"; "#
    );
    assert_eq!(strip_comment("let x = 1; // crate::render"), "let x = 1; ");
}

#[test]
fn a_gated_item_with_literals_ends_where_its_brackets_do() {
    let src = "#[cfg(test)]\nfn f() {\n    json!({ \"uri\": \"rusty://nope.md\" });\n    \
               format!(\"fn {name}(\");\n}\nfn g() {}";
    assert_eq!(end(src, 1), 5);
}

#[test]
fn a_string_open_across_lines_hides_its_brackets() {
    let src =
        "#[cfg(test)]\nfn f() {\n    let s = r#\"return T({\n        a = { 1 },\n    })\"#;\n    \
               let t = \"(\n    \";\n}\nfn g() {}";
    assert_eq!(end(src, 1), 8);
    let mut open = Open::None;
    assert_eq!(brackets("x(r#\"{", &mut open), ("x(r#\"{", 1));
    assert_eq!(open, Open::Raw(1));
    assert_eq!(brackets("}\"#) // }", &mut open), ("}\"#) ", -1));
    assert_eq!(open, Open::None);
}
