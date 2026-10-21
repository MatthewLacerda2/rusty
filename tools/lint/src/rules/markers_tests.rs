use super::{check, RULE, WORDS};

#[test]
fn flags_every_marker_word_standing_alone() {
    for word in WORDS {
        let src = format!("fn a() {{}}\n// {word}: later\n");
        let found = check("src/a.rs", &src);
        assert_eq!(found.len(), 1, "{word}");
        assert_eq!((found[0].line, found[0].rule), (2, RULE));
    }
}

#[test]
fn passes_clean_code_and_longer_words() {
    let todo = WORDS[0];
    assert!(check("src/a.rs", "// a finished thing\n").is_empty());
    let src = format!("let {todo}S = 1;\nlet MY_{todo} = 2;\nlet x{todo} = 3;\n");
    assert!(check("src/a.rs", &src).is_empty());
}
