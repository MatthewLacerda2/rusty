//! Unit tests for the typed animator parameters (#314): typed set/get round-trips,
//! the wrong-type no-op rule, trigger latch/consume semantics, and the store's
//! deterministic (name-sorted) iteration order.

use super::parameters::AnimatorParameter;
use super::AnimatorComponent;

#[test]
fn typed_set_get_round_trips_every_kind() {
    let mut anim = AnimatorComponent::default();
    assert!(anim.set_bool("isGrounded", true));
    assert!(anim.set_float("speed", 4.2));
    assert!(anim.set_int("weapon", 2));
    assert!(anim.set_trigger("Jump"));
    assert_eq!(anim.get_bool("isGrounded"), Some(true));
    assert_eq!(anim.get_float("speed"), Some(4.2));
    assert_eq!(anim.get_int("weapon"), Some(2));
    assert!(anim.trigger_is_set("Jump"));

    // Same-type overwrite updates in place.
    assert!(anim.set_float("speed", 0.0));
    assert_eq!(anim.get_float("speed"), Some(0.0));
    assert_eq!(anim.parameters.len(), 4, "overwrite adds no entry");
}

#[test]
fn wrong_typed_set_is_a_refused_no_op_not_a_panic() {
    let mut anim = AnimatorComponent::default();
    anim.set_bool("isGrounded", true);
    assert!(
        !anim.set_float("isGrounded", 1.0),
        "a name bound to Bool refuses a Float write"
    );
    assert_eq!(
        anim.get_bool("isGrounded"),
        Some(true),
        "the refused write leaves the value untouched"
    );
    assert!(!anim.set_trigger("isGrounded"), "and refuses a Trigger too");
}

#[test]
fn typed_get_on_missing_or_wrong_type_is_none() {
    let mut anim = AnimatorComponent::default();
    anim.set_int("weapon", 1);
    assert_eq!(anim.get_bool("weapon"), None);
    assert_eq!(anim.get_float("weapon"), None);
    assert_eq!(anim.get_int("nope"), None);
    assert!(!anim.trigger_is_set("nope"));
}

#[test]
fn trigger_latches_until_consumed_then_clears() {
    let mut anim = AnimatorComponent::default();
    anim.set_trigger("Jump");
    // Latched: non-consuming checks may read it any number of times.
    assert!(anim.trigger_is_set("Jump"));
    assert!(anim.trigger_is_set("Jump"));
    // A fired transition consumes it exactly once (#316's read-site).
    assert!(anim.consume_trigger("Jump"));
    assert!(
        !anim.trigger_is_set("Jump"),
        "consume auto-clears the latch"
    );
    assert!(
        !anim.consume_trigger("Jump"),
        "a cleared trigger stays clear"
    );
    // The parameter itself survives as Trigger(false) — declared, not deleted.
    assert_eq!(
        anim.parameters.get("Jump"),
        Some(&AnimatorParameter::Trigger(false))
    );
    // Re-latching works.
    anim.set_trigger("Jump");
    assert!(anim.consume_trigger("Jump"));
}

#[test]
fn consume_trigger_on_missing_or_wrong_type_is_false() {
    let mut anim = AnimatorComponent::default();
    anim.set_bool("isGrounded", true);
    assert!(!anim.consume_trigger("nope"));
    assert!(!anim.consume_trigger("isGrounded"));
    assert_eq!(anim.get_bool("isGrounded"), Some(true), "left untouched");
}

#[test]
fn iteration_order_is_name_sorted_regardless_of_insertion_order() {
    // The future evaluator walks the store on the fixed step; a BTreeMap keeps that
    // walk deterministic. Insert out of order, iterate sorted.
    let mut anim = AnimatorComponent::default();
    anim.set_float("speed", 1.0);
    anim.set_bool("aiming", false);
    anim.set_trigger("Jump");
    anim.set_int("weapon", 0);
    let names: Vec<&str> = anim.parameters.keys().map(String::as_str).collect();
    assert_eq!(names, ["Jump", "aiming", "speed", "weapon"]);
}
