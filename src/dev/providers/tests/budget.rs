//! The per-project budget: persisted, cumulative, and a hard stop (#383).

use super::*;

#[test]
fn a_project_without_a_ledger_starts_at_the_default_budget() {
    let ledger = Ledger::load(&scratch("default").join("provider_budget.json")).unwrap();
    assert_eq!(ledger.budget_cents, ledger::DEFAULT_BUDGET_CENTS);
    assert_eq!(ledger.spent_cents, 0);
}

#[test]
fn recorded_spending_persists_and_accumulates() {
    let path = scratch("accumulate")
        .join("nested")
        .join("provider_budget.json");
    for _ in 0..3 {
        let permit = permit(false, Provider::ElevenLabs, 40, &keyed(), &path).unwrap();
        permit.record(&path).unwrap();
    }
    let ledger = Ledger::load(&path).unwrap();
    assert_eq!(ledger.spent_cents, 120);
    assert_eq!(ledger.budget_cents, ledger::DEFAULT_BUDGET_CENTS);
}

#[test]
fn a_call_that_would_cross_the_budget_names_both_numbers_and_the_overage() {
    let path = scratch("over").join("provider_budget.json");
    Ledger {
        budget_cents: 100,
        spent_cents: 90,
    }
    .save(&path)
    .unwrap();
    let refusal = permit(false, Provider::ElevenLabs, 25, &keyed(), &path).unwrap_err();
    let expected = Refusal::OverBudget {
        estimate_cents: 25,
        spent_cents: 90,
        budget_cents: 100,
        over_cents: 15,
        file: path.display().to_string(),
    };
    assert_eq!(refusal, expected);
    let message = refusal.to_string();
    for figure in ["25", "90", "100-cent", "by 15", "budget_cents"] {
        assert!(
            message.contains(figure),
            "{figure:?} missing from {message:?}"
        );
    }
}

#[test]
fn a_call_that_lands_exactly_on_the_budget_goes_ahead() {
    let path = scratch("exact").join("provider_budget.json");
    Ledger {
        budget_cents: 100,
        spent_cents: 90,
    }
    .save(&path)
    .unwrap();
    assert!(permit(false, Provider::ElevenLabs, 10, &keyed(), &path).is_ok());
}

#[test]
fn a_ledger_that_does_not_parse_refuses_rather_than_resets() {
    let path = scratch("corrupt").join("provider_budget.json");
    std::fs::write(&path, "{ not json").unwrap();
    let refusal = permit(false, Provider::ElevenLabs, 1, &keyed(), &path).unwrap_err();
    assert!(matches!(refusal, Refusal::Ledger(_)), "got {refusal:?}");
}
