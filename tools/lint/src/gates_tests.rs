//! Every row of [`super::GATES`] is wired: its entry point, run over a fixture it
//! must flag, reports that fixture's planted file. Each gate's own tests hold its
//! judgement; this holds that the row `main` dispatches still reaches it, so a gate
//! emptied to "no findings", or a pass it stopped calling, fails here.
//!
//! The cases are keyed by the row's flag, so a new row with no case fails the test
//! naming the gate. A gate with several passes (the size gate's files and agent
//! docs, the layer gate's table and its external crates) has a case per pass.

use super::GATES;
use crate::repo::scaffold::Repo;

/// A must-flag case: the files of a scratch repository, and text one of the
/// gate's findings must contain.
type Case = (Vec<(&'static str, String)>, &'static str);

/// The must-flag cases of the gate selected by `flag`, `None` when it has none.
fn cases_for(flag: &str) -> Option<Vec<Case>> {
    Some(match flag {
        "" => vec![
            (vec![("src/long.rs", "\n".repeat(301))], "src/long.rs"),
            (
                vec![("CLAUDE.md", "# CLAUDE.md\n\nFixed on 2026-01-02.\n".into())],
                "CLAUDE.md",
            ),
        ],
        "--components" => vec![(vec![("README.md", "".into())], "NO_REGISTRY")],
        "--determinism" => vec![(
            vec![(
                "crates/app/src/lib.rs",
                "#![allow(clippy::disallowed_methods)]\n".into(),
            )],
            "crates/app/src/lib.rs",
        )],
        "--layers" => vec![
            // the table against the tree: a declared module that is absent
            (
                vec![("Cargo.toml", "[package]\nname = \"t\"\n".into())],
                "MISSING_MODULE physics",
            ),
            // `externs`: a confined crate named outside its confinement, which
            // only reading the code (`uses`) finds
            (
                vec![
                    (
                        "Cargo.toml",
                        "[package]\nname = \"t\"\n\n[dependencies]\negui = \"1\"\n".into(),
                    ),
                    ("crates/physics/src/lib.rs", "use egui::Ui;\n".into()),
                ],
                "CONFINED_CRATE crates/physics/src/lib.rs:1",
            ),
        ],
        "--parity" => vec![(vec![("README.md", "".into())], "NO_REGISTRY")],
        "--rules" => vec![(
            vec![("src/a.rs", "fn f() {} // lint: allow(markers)\n".into())],
            "src/a.rs",
        )],
        _ => return None,
    })
}

#[test]
fn every_gate_reports_its_must_flag_cases_through_its_row() {
    let mut broken = Vec::new();
    for (flag, check) in GATES {
        let Some(cases) = cases_for(flag) else {
            broken.push(format!(
                "gate `{flag}` has no must-flag case in `cases_for`"
            ));
            continue;
        };
        for (i, (files, names)) in cases.iter().enumerate() {
            let tag = format!("gate{}-{i}", flag.trim_start_matches('-'));
            let repo = Repo::new(&tag, true);
            for (path, body) in files {
                repo.file(path, body);
            }
            repo.git(&["add", "-A"]);
            let verdict = check(&repo.0, &[]);
            if !verdict.findings.iter().any(|f| f.contains(names)) {
                broken.push(format!(
                    "gate `{flag}` did not report `{names}`: {:?}",
                    verdict.findings
                ));
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn a_flag_selects_its_row_and_no_flag_the_size_gate() {
    for (flag, check) in GATES {
        let args = [flag.to_string()];
        let args: &[String] = if flag.is_empty() { &[] } else { &args };
        assert_eq!(
            super::select(args) as usize,
            *check as usize,
            "`{flag}` selects another gate"
        );
    }
}
