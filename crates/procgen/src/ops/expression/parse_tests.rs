//! The parser: precedence and associativity, each error's column, and the caps
//! that keep untrusted text from making it recurse or allocate without bound.

use super::eval_tests::value;
use super::*;

fn err(src: &str) -> SyntaxError {
    compile(src).expect_err(src)
}

#[test]
fn precedence_and_associativity_follow_maths() {
    let cases = [
        ("2 ^ 3 ^ 2", 512.0),
        ("-2 ^ 2", -4.0),
        ("(-2) ^ 2", 0.0), // `math`'s power reads a negative base as 0
        ("2 ^ -1", 0.5),
        ("1 + 2 * 3", 7.0),
        ("(1 + 2) * 3", 9.0),
        ("8 / 4 / 2", 1.0),
        ("8 - 4 - 2", 2.0),
        ("1 + 1 < 3", 1.0),
        ("1 < 2 == 1", 1.0),
        ("--3", 3.0),
        ("+3", 3.0),
        ("2 * -3", -6.0),
        (".5 + 1.5e1", 15.5),
        ("pi", std::f32::consts::PI),
        ("tau / 2", std::f32::consts::PI),
    ];
    for (src, want) in cases {
        assert_eq!(value(src), want, "{src}");
    }
}

#[test]
fn every_error_names_its_column_and_the_fix() {
    let e = err("0.5 + sin(2) + sine(u)");
    assert_eq!(
        (e.what.as_str(), e.column),
        ("unknown function \"sine\"", Some(16))
    );
    assert!(e.fix.starts_with("functions are min, max"), "{}", e.fix);
    let cases = [
        ("u + x", "unknown name \"x\"", 5),
        ("u(2)", "\"u\" is not a function", 1),
        ("sin", "function \"sin\" is missing its arguments", 1),
        (
            "clamp(u, 0)",
            "function \"clamp\" takes 3 arguments, but 2 given",
            1,
        ),
        ("a.x", "input a has no channel \"x\"", 3),
        ("u % 2", "unexpected character '%'", 3),
        ("(u + 1", "the formula ends early", 7),
        ("u v", "unexpected name \"v\"", 3),
        ("u + * v", "unexpected \"*\"", 5),
        ("1e99", "the number 1e99 is out of range", 1),
        ("é + x", "unexpected character 'é'", 1),
        ("u + é", "unexpected character 'é'", 5),
    ];
    for (src, what, column) in cases {
        let e = err(src);
        assert_eq!((e.what.as_str(), e.column), (what, Some(column)), "{src}");
        assert!(!e.fix.is_empty(), "{src} names no fix");
    }
    assert_eq!(err("  ").what, "the formula is empty");
}

#[test]
fn a_column_counts_characters_past_a_multibyte_one() {
    // The lexer refuses the `é` itself, so its column is the one any later error
    // would be measured from: characters, not bytes.
    assert_eq!(err("1 + 2 + é").column, Some(9));
}

#[test]
fn the_named_inputs_and_channels_are_recorded() {
    let p = compile("a + c.g * b.a").unwrap();
    assert_eq!(p.inputs(), [true, true, true, false]);
    assert_eq!(compile("u * k4").unwrap().inputs(), [false; 4]);
}

#[test]
fn deep_nesting_is_refused_not_overflowed() {
    for open in ["(", "-", "sin(", "2 ^ "] {
        let deep = format!("{}1", open.repeat(1000));
        let e = err(&deep);
        assert!(e.what.contains("nests deeper than 64"), "{open}: {e:?}");
        assert_eq!(e.fix, "split it across nodes");
    }
    // At the cap it still compiles.
    let ok = format!(
        "{}1{}",
        "(".repeat(MAX_DEPTH - 1),
        ")".repeat(MAX_DEPTH - 1)
    );
    assert_eq!(value(&ok), 1.0);
}

#[test]
fn a_long_formula_is_capped_and_a_long_chain_under_it_runs() {
    let e = err(&"1+".repeat(MAX_LEN));
    assert_eq!(e.column, None);
    assert!(
        e.what.contains("characters long") && e.fix.contains("4096"),
        "{e:?}"
    );
    // A flat chain does not recurse: 2000 additions compile and run.
    let chain = format!("0{}", "+1".repeat(2000));
    assert_eq!(value(&chain), 2000.0);
    let p = compile(&chain).unwrap();
    assert_eq!(
        p.stack, 2,
        "a left-leaning chain holds two values at a time"
    );
}

#[test]
fn malformed_text_never_panics() {
    let junk = [
        "", ")", "(", ",", ".", "a.", "a..r", "1..2", "1e", "1e+", "sin(,)", "min(1,)", "((((",
        "))))", "u ^", "- ", "k5", "k0", "a.rr", "ab", "e", "!", "!=", "=", "1 = 2", "\u{0}",
        "\t\n",
    ];
    for src in junk {
        let _ = compile(src);
    }
}
