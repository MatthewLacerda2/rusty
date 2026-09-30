//! Frame stats in the headless harness (#433): the timing probe records stages,
//! systems and world counters every tick; `results.json` keeps only the deterministic
//! counts, `stats.json` everything; budgets become expectations; `Debug.Stats()` reads
//! the same numbers. Gated on `dev`.

use rusty::dev::harness::Harness;

fn harness(tag: &str) -> (Harness, std::path::PathBuf) {
    let out = std::env::temp_dir().join(format!("rusty_stats_{tag}_{}", std::process::id()));
    (Harness::new(&out, ""), out)
}

#[test]
fn a_run_records_stage_system_and_world_stats() {
    let (h, out) = harness("run");
    h.step(30);
    let stats = h.stats.borrow().clone();
    assert_eq!(stats.frames, 30);
    let fixed = stats.get("fixed_update_ms").expect("stage timing recorded");
    assert_eq!(fixed.samples, 30);
    assert!(stats.get("entities").is_some_and(|s| s.max > 0.0));
    assert!(
        stats.systems.contains_key("step_physics"),
        "{:?}",
        stats.systems.keys()
    );
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn results_carry_counts_and_stats_json_carries_timings() {
    let (h, out) = harness("files");
    h.step(5);
    let results_path = h.write_results().unwrap();
    let read = |p: &std::path::Path| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    };
    let results = read(&results_path);
    assert_eq!(results["stats"]["frames"], 5);
    assert!(results["stats"]["metrics"].get("entities").is_some());
    assert!(results["stats"]["metrics"].get("fixed_update_ms").is_none());
    let full = read(&out.join("stats.json"));
    assert!(full["metrics"].get("fixed_update_ms").is_some());
    assert!(full["systems"].get("update_scripts").is_some());
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn budgets_pass_fail_and_name_unmeasured_metrics() {
    let (mut h, out) = harness("budget");
    h.step(3);
    let b = |k: &str, v: f64| (k.to_string(), v);
    assert!(h.assert_budget(&[b("entities", 1e6), b("fixed_update_ms", 1e6)]));
    assert!(h.all_passed());
    assert!(!h.assert_budget(&[b("entities", 0.0)]));
    assert!(
        !h.assert_budget(&[b("draw_calls", 1.0)]),
        "no screenshot, no render counters"
    );
    let last = &h.expectations.last().unwrap().message;
    assert!(last.contains("never measured"), "{last}");
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn debug_stats_reads_the_same_numbers() {
    let (h, out) = harness("debug");
    h.step(4);
    let frames = h
        .world
        .borrow()
        .script_manager()
        .eval("return Debug.Stats().frames")
        .unwrap();
    assert_eq!(frames, "4");
    let _ = std::fs::remove_dir_all(out);
}

#[test]
fn a_scenario_asserts_budgets_through_the_harness_table() {
    let dir = std::env::temp_dir().join(format!("rusty_stats_lua_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scenario = dir.join("budget.lua");
    std::fs::write(
        &scenario,
        "Harness.Step(10)\n\
         Harness.Expect(Harness.Stats().frames == 10, 'stats count frames')\n\
         Harness.AssertBudget{ entities = 1000, scripts = 1000 }\n\
         Harness.AssertBudget{ entities = 0 }\n",
    )
    .unwrap();
    let report = rusty::dev::scenario::run(&scenario, &dir).unwrap();
    assert!(!report.passed, "the zero-entity budget must fail the run");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report.results_path).unwrap()).unwrap();
    let passed: Vec<bool> = results["expectations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["passed"].as_bool().unwrap())
        .collect();
    assert_eq!(passed, [true, true, true, false]);
    let _ = std::fs::remove_dir_all(dir);
}
