use super::GATES;
use crate::repo::scaffold::Repo;

type Case = (Vec<(&'static str, String)>, &'static str);

const REGISTRY: (&str, &str) = (
    "crates/components/src/registry.rs",
    "macro_rules! first_class {\n    collider: physics::ColliderComponent => Collider [];\n}\n",
);

fn files(list: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    list.iter().map(|(p, b)| (*p, b.to_string())).collect()
}

fn cases_for(flag: &str) -> Option<Vec<Case>> {
    let no_registry = (files(&[("README.md", "")]), "NO_REGISTRY");
    Some(match flag {
        "" => vec![
            (
                vec![("src/long.rs", "\n".repeat(301))],
                "FILE_TOO_LONG src/long.rs ",
            ),
            (
                files(&[("CLAUDE.md", "# CLAUDE.md\n\nFixed on 2026-01-02.\n")]),
                "CLAUDE.md:3: [history]",
            ),
            (
                vec![(
                    "build.rs",
                    format!("// {}\nfn main() {{}}\n", "w ".repeat(101)),
                )],
                "build.rs:1: [comment-cap]",
            ),
            (
                files(&[("games/a/main.lua", "x = 1 -- since #1234\n")]),
                "games/a/main.lua:1: [comment-history]",
            ),
            (
                files(&[(
                    "src/wide.rs",
                    &format!("// {}\nfn f() {{}}\n", "wide ".repeat(25)),
                )]),
                "src/wide.rs:1: [comment-width]",
            ),
        ],
        "--components" => vec![
            no_registry,
            (files(&[REGISTRY]), "INCOMPLETE_COMPONENT `collider`"),
        ],
        "--determinism" => vec![
            (
                files(&[(
                    "crates/app/src/lib.rs",
                    "#![allow(clippy::disallowed_methods)]\n",
                )]),
                "OPT_OUT crates/app/src/lib.rs:1",
            ),
            (
                files(&[("crates/app/Cargo.toml", "[package]\nname = \"app\"\n")]),
                "UNINHERITED crates/app/Cargo.toml ",
            ),
            (
                files(&[(
                    "crates/app/src/lib.rs",
                    "#[expect(clippy::unwrap_used, reason = \"r\")]\nfn f() {}\n",
                )]),
                "ESCAPE crates/app/src/lib.rs:1",
            ),
        ],
        "--layers" => vec![
            (
                files(&[("Cargo.toml", "[package]\nname = \"t\"\n")]),
                "MISSING_MODULE physics",
            ),
            (
                files(&[
                    (
                        "Cargo.toml",
                        "[package]\nname = \"t\"\n\n[dependencies]\negui = \"1\"\n",
                    ),
                    ("crates/physics/src/lib.rs", "use egui::Ui;\n"),
                ]),
                "CONFINED_CRATE crates/physics/src/lib.rs:1",
            ),
        ],
        "--parity" => vec![
            no_registry,
            (
                files(&[
                    REGISTRY,
                    (
                        "src/editor/inspector/components/c.rs",
                        "let c = w.collider_mut(id);\n",
                    ),
                ]),
                "PARITY_DRIFT `collider`",
            ),
        ],
        "--rules" => vec![
            (
                files(&[("src/a.rs", "fn f() {} // lint: allow(markers)\n")]),
                "src/a.rs:1: [allow]",
            ),
            (
                files(&[("crates/app/src/b.rs", concat!("// TO", "DO: later\n"))]),
                "crates/app/src/b.rs:1: [markers]",
            ),
        ],
        _ => return None,
    })
}

/// At a path with a `tests` segment of its own: a gate judging a file by its absolute
/// path would cap every file as a test.
fn plant(tag: &str, files: &[(&str, String)]) -> (Repo, Repo) {
    let holder = Repo::new(tag, false);
    let repo = Repo(holder.0.join("tests"));
    std::fs::create_dir_all(&repo.0).expect("repo dir");
    repo.git(&["init", "-q"]);
    for (path, body) in files {
        repo.file(path, body);
    }
    repo.git(&["add", "-A"]);
    (repo, holder)
}

#[test]
fn every_gate_reports_its_must_flag_cases_through_its_row() {
    let mut broken = Vec::new();
    for (check, flag) in GATES {
        let Some(cases) = cases_for(flag) else {
            broken.push(format!(
                "gate `{flag}` has no must-flag case in `cases_for`"
            ));
            continue;
        };
        for (i, (files, names)) in cases.iter().enumerate() {
            let (repo, _holder) =
                plant(&format!("gate{}-{i}", flag.trim_start_matches('-')), files);
            let verdict = check(&repo.0, &[]);
            if !verdict.findings.iter().any(|f| f.contains(names)) {
                broken.push(format!(
                    "gate `{flag}` did not report `{names}`: {:?}",
                    verdict.findings
                ));
            }
            let base = repo.0.to_string_lossy().into_owned();
            for f in verdict.findings.iter().filter(|f| f.contains(&base)) {
                broken.push(format!(
                    "gate `{flag}` named the repository's own path: {f}"
                ));
            }
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn a_file_is_capped_by_its_path_under_base_not_above_it() {
    // 200 lines: past the test cap, inside the source cap.
    let (repo, _holder) = plant("gate-base-cap", &[("src/mid.rs", "\n".repeat(200))]);
    for args in [vec![], vec!["src/mid.rs".to_string()]] {
        let found = crate::size::verdict(&repo.0, &args).findings;
        assert!(
            !found.iter().any(|f| f.contains("FILE_TOO_LONG")),
            "{found:?}"
        );
    }
}

#[test]
fn where_git_cannot_list_the_tree_the_report_says_so() {
    let dir = Repo::new("gate-no-git", false);
    let verdict = crate::size::verdict(&dir.0, &[]);
    assert!(
        verdict.body.contains("committable files"),
        "{}",
        verdict.body
    );
    assert!(
        verdict.body.contains("agent docs, so they go unchecked"),
        "{}",
        verdict.body
    );
}

#[test]
fn a_missing_registry_is_advised_as_that_alone() {
    let (repo, _holder) = plant("gate-no-registry", &[]);
    for check in [crate::components::verdict, crate::parity::verdict] {
        let advice = check(&repo.0, &[]).advice;
        assert!(advice.contains("discover.rs"), "{advice}");
        assert!(
            !advice.contains("waive") && !advice.contains("Route"),
            "{advice}"
        );
    }
}

#[test]
fn a_flag_selects_its_row_and_no_flag_the_size_gate() {
    for (check, flag) in GATES {
        let args = [flag.to_string()];
        let args: &[String] = if flag.is_empty() { &[] } else { &args };
        let picked = super::select(args).expect("one flag");
        assert_eq!(
            picked as usize, *check as usize,
            "`{flag}` selects another gate"
        );
    }
    let files = ["a.rs".to_string()];
    assert_eq!(
        super::select(&files).expect("files") as usize,
        crate::size::verdict as super::Check as usize
    );
}

#[test]
fn two_gate_flags_are_refused() {
    for pair in [["--rules", "--dead-pub"], ["--layers", "--rules"]] {
        let args: Vec<String> = pair.iter().map(|a| a.to_string()).collect();
        let why = super::select(&args).expect_err("two flags");
        assert!(why.contains(pair[0]) && why.contains(pair[1]), "{why}");
    }
    let dead = [super::DEAD_PUB.to_string()];
    assert!(super::select(&dead).is_ok());
}
