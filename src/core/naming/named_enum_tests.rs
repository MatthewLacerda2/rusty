//! One test for every enum `named_enum!` declares (#1329): what the macro writes
//! is the same for all of them, so it is proven once, here.

crate::core::named_enum! {
    /// A test enum: one plain variant, one renamed, one with aliases.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Probe {
        Plain = "Plain",
        Renamed = "home_theater",
        Aliased = "Custom" | "User" | "Hand Set",
    }
}

#[test]
fn all_lists_every_variant_in_declaration_order() {
    assert_eq!(Probe::ALL, [Probe::Plain, Probe::Renamed, Probe::Aliased]);
}

#[test]
fn name_is_the_declared_spelling_and_display_prints_it() {
    let names: Vec<_> = Probe::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(names, ["Plain", "home_theater", "Custom"]);
    assert_eq!(Probe::Renamed.to_string(), "home_theater");
}

#[test]
fn parse_round_trips_every_name_in_any_case() {
    for p in Probe::ALL {
        for spelling in [
            p.name().to_string(),
            p.name().to_ascii_lowercase(),
            p.name().to_ascii_uppercase(),
        ] {
            assert_eq!(Probe::parse(&spelling), Some(p), "{spelling}");
        }
    }
}

#[test]
fn names_joins_every_name_for_an_error() {
    assert_eq!(Probe::names(), "Plain, home_theater, Custom");
}

#[test]
fn unique_refuses_two_spellings_that_differ_only_in_case() {
    assert!(super::unique(&["Plain", "home_theater", "Custom", "User"]));
    assert!(!super::unique(&["Plain", "Custom", "PLAIN"]));
    assert!(!super::unique(&["A", "a"]));
    assert!(super::unique(&[]));
}

#[test]
fn parse_reads_aliases_and_refuses_anything_else() {
    assert_eq!(Probe::parse("user"), Some(Probe::Aliased));
    assert_eq!(Probe::parse("HAND SET"), Some(Probe::Aliased));
    for unknown in ["", "Renamed", "Plai", "Plain ", "hometheater"] {
        assert_eq!(Probe::parse(unknown), None, "{unknown:?}");
    }
}

#[test]
fn a_stored_file_holds_the_declared_name() {
    let stored: Vec<_> = Probe::ALL.map(|p| serde_json::to_value(p).unwrap()).into();
    assert_eq!(stored, ["Plain", "home_theater", "Custom"]);
}

#[test]
fn a_stored_name_reads_in_any_case_and_through_its_aliases() {
    let read = |s: &str| serde_json::from_value::<Probe>(serde_json::json!(s));
    for p in Probe::ALL {
        assert_eq!(read(p.name()).unwrap(), p);
        assert_eq!(read(&p.name().to_ascii_uppercase()).unwrap(), p);
    }
    assert_eq!(read("hand set").unwrap(), Probe::Aliased);
    let error = read("Renamed").unwrap_err().to_string();
    assert!(error.contains("unknown variant `Renamed`"), "{error}");
    assert!(error.contains("`home_theater`"), "lists the names: {error}");
}
