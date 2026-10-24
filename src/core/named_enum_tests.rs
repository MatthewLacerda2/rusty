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
fn parse_reads_aliases_and_refuses_anything_else() {
    assert_eq!(Probe::parse("user"), Some(Probe::Aliased));
    assert_eq!(Probe::parse("HAND SET"), Some(Probe::Aliased));
    for unknown in ["", "Renamed", "Plai", "Plain ", "hometheater"] {
        assert_eq!(Probe::parse(unknown), None, "{unknown:?}");
    }
}
