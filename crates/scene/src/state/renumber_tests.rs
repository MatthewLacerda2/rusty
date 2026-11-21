use super::inventory::fields;
use super::{Class, Field, Keyed};
use crate::runtime::renumber::{Pass, RENUMBERED};

fn disagreements(fields: &[Field], renumbered: &[&str]) -> Vec<String> {
    let owed: Vec<&str> = fields
        .iter()
        .filter(|f| f.class.saved() && f.keyed != Keyed::No)
        .map(|f| f.name)
        .collect();
    let missing = owed.iter().filter(|name| !renumbered.contains(name));
    let missing = missing.map(|name| {
        format!(
            "`{name}` is saved and keyed by entity: add it to `RENUMBERED`, as \
             `Pass::Document` if the document carries it, `Pass::Sidecar` if a \
             separate file does"
        )
    });
    let extra = renumbered.iter().filter(|name| !owed.contains(name));
    let extra = extra.map(|name| format!("`{name}` is renumbered but no saved keyed field"));
    missing.chain(extra).collect()
}

fn renumbered() -> Vec<&'static str> {
    RENUMBERED.iter().map(|(name, _)| *name).collect()
}

#[test]
fn every_saved_field_keyed_by_entity_follows_a_renumbering() {
    assert_eq!(
        disagreements(&fields(), &renumbered()),
        Vec::<String>::new()
    );
}

#[test]
fn the_document_renames_its_own_ids_and_a_separate_file_follows_once_in() {
    let passes: Vec<(&str, &str)> = RENUMBERED
        .iter()
        .map(|(name, pass)| match pass {
            Pass::Document(_) => (*name, "document"),
            Pass::Sidecar(_) => (*name, "sidecar"),
        })
        .collect();
    assert_eq!(
        passes,
        [
            ("selected_entity_id", "document"),
            ("lightmaps", "document"),
            ("occlusion", "sidecar"),
        ]
    );
}

#[test]
fn a_keyed_field_the_renumbering_skips_is_flagged_and_an_unkeyed_one_passes() {
    let field = |name, keyed| Field {
        name,
        class: Class::BakeOutput,
        keyed,
        saved_as: &[],
    };
    let mut mocked = fields();
    mocked.push(field("mock_bake", Keyed::Dropped));
    let flagged = disagreements(&mocked, &renumbered());
    assert_eq!(flagged.len(), 1, "{flagged:?}");
    assert!(flagged[0].contains("`mock_bake`"), "{flagged:?}");
    let mut unkeyed = fields();
    unkeyed.push(field("mock_settings", Keyed::No));
    assert!(disagreements(&unkeyed, &renumbered()).is_empty());
    let mut stale = renumbered();
    stale.push("gone");
    assert_eq!(disagreements(&fields(), &stale).len(), 1);
}
