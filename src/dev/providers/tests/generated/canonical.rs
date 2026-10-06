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

#[test]
fn today_counts_whole_days_since_the_epoch() {
    let file = root("today").join("stamp");
    std::fs::write(&file, b"").unwrap();
    let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
    let day = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86_400;
    let today = today();
    // Either side of midnight between the write and the read.
    assert!(
        today == civil_date(day) || today == civil_date(day + 1),
        "today() is {today}, the file was written on {}",
        civil_date(day)
    );
}

#[test]
fn civil_date_matches_a_day_by_day_gregorian_walk_to_2401() {
    let leap = |y: u64| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let (mut year, mut month, mut day, mut n) = (1970, 1, 1, 0);
    while year < 2401 {
        assert_eq!(civil_date(n), format!("{year:04}-{month:02}-{day:02}"));
        let length = match month {
            2 if leap(year) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        day += 1;
        if day > length {
            (day, month) = (1, month + 1);
        }
        if month > 12 {
            (month, year) = (1, year + 1);
        }
        n += 1;
    }
    assert_eq!(n, 157_420, "the walk covered every day to 2401-01-01");
}
