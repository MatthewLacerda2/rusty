use super::*;

#[test]
fn values_are_typed_and_written_as_a_lua_table() {
    let params = Params::parse(["size=30", "bake=true", "mode=dust-2.v1", "n=-0.5"]).unwrap();
    assert_eq!(
        params.to_lua(),
        r#"{ size = 30, bake = true, mode = "dust-2.v1", n = -0.5 }"#
    );
}

#[test]
fn a_whole_number_is_a_lua_integer_and_a_huge_one_a_float() {
    let params = Params::parse(["seed=3.0", "far=1e300"]).unwrap();
    assert_eq!(params.to_lua(), "{ seed = 3, far = 1e300 }");
}

#[test]
fn no_params_is_an_empty_table() {
    assert_eq!(Params::default().to_lua(), "{}");
}

#[test]
fn a_value_that_would_need_escaping_is_refused() {
    for bad in [r#"name=a"b"#, "name=a b", "name=", "name=x]]"] {
        assert!(Params::parse([bad]).is_err(), "{bad} is refused");
    }
}

#[test]
fn a_key_must_be_a_lua_name_and_a_pair_needs_its_equals() {
    for bad in ["1size=3", "si ze=3", "=3", "size"] {
        assert!(Params::parse([bad]).is_err(), "{bad} is refused");
    }
}

#[test]
fn a_non_finite_number_reaches_the_recipe_as_a_word() {
    // `inf` parses as an f64 but is no number a recipe can use; the word is plain,
    // so it reaches the recipe as a string, for the recipe to refuse.
    let params = Params::parse(["x=inf"]).unwrap();
    assert_eq!(params.to_lua(), r#"{ x = "inf" }"#);
    assert!(Params::parse(["x=NaN"]).is_ok());
}
