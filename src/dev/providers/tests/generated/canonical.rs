//! The brief's canonical form, its digest, and the sidecar beside each file (#384).

use serde::Serialize;

use super::super::super::generated::*;
use super::{line, root, Line};

#[test]
fn the_canonical_form_round_trips_to_the_same_digest_across_a_save_and_a_load() {
    let mut brief = line("Hold the line");
    brief.seed = Some(42);
    let saved = serde_json::to_string_pretty(&brief).unwrap();
    let loaded: Line = serde_json::from_str(&saved).unwrap();
    assert_eq!(digest(&loaded).unwrap(), digest(&brief).unwrap());
    // A file that lists its keys in another order is the same brief.
    let reordered =
        r#"{"tags":{"mood":"grim"},"speed":1.1,"seed":42,"voice":"v1","text":"Hold the line"}"#;
    let reloaded: Line = serde_json::from_str(reordered).unwrap();
    assert_eq!(digest(&reloaded).unwrap(), digest(&brief).unwrap());
}

#[test]
fn a_null_is_absent_so_a_new_optional_field_keeps_old_addresses() {
    #[derive(Serialize)]
    struct Grown<'a> {
        #[serde(flatten)]
        line: &'a Line,
        style: Option<String>,
    }
    impl Brief for Grown<'_> {
        fn kind(&self) -> &'static str {
            "speech"
        }
        fn extension(&self) -> &'static str {
            "mp3"
        }
    }
    let old = line("Contact!");
    assert_eq!(
        digest(&Grown {
            line: &old,
            style: None
        })
        .unwrap(),
        digest(&old).unwrap()
    );
    let styled = Grown {
        line: &old,
        style: Some("shout".into()),
    };
    assert_ne!(digest(&styled).unwrap(), digest(&old).unwrap());
}

#[test]
fn every_field_and_the_kind_move_the_address() {
    let base = line("Go");
    let mut faster = base.clone();
    faster.speed = 1.2;
    let mut seeded = base.clone();
    seeded.seed = Some(1);
    assert_ne!(digest(&faster).unwrap(), digest(&base).unwrap());
    assert_ne!(digest(&seeded).unwrap(), digest(&base).unwrap());
    let value = canonical(&base).unwrap();
    assert_ne!(digest_of("sfx", &value), digest_of("speech", &value));
}

#[test]
fn the_sidecar_records_brief_date_and_estimate_and_re_hashes_to_the_name() {
    let root = root("sidecar");
    let brief = line("Grenade!");
    let path = store(&root, &brief, b"mp3", 12, "2026-10-05").unwrap();
    let sidecar = Sidecar::load(&root.join(&path)).unwrap();
    assert_eq!(
        (sidecar.kind.as_str(), sidecar.estimate_cents),
        ("speech", 12)
    );
    assert_eq!(sidecar.generated_on, "2026-10-05");
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(name.contains(&digest_of(&sidecar.kind, &sidecar.brief)));
    let text = std::fs::read_to_string(Sidecar::path_for(&root.join(&path))).unwrap();
    assert!(
        !text.contains("key"),
        "a sidecar never records a key: {text}"
    );
}

#[test]
fn today_is_a_utc_calendar_day() {
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(20_731), "2026-10-05");
    assert_eq!(civil_date(11_016), "2000-02-29");
    assert_eq!(today().len(), 10);
}
