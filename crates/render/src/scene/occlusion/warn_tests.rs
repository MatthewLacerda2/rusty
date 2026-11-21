use super::warning_due;
use rusty_scene::revision::Revision;

#[test]
fn a_stale_bake_is_said_once_and_again_when_an_occluder_breaks() {
    let pvs = Revision::default();
    assert!(!warning_due(None, pvs, (0, 0)), "nothing changed");
    assert!(warning_due(None, pvs, (1, 0)), "a prop moved");
    let said = Some((pvs, false));
    assert!(!warning_due(said, pvs, (2, 0)), "said once");
    assert!(warning_due(said, pvs, (3, 1)), "an occluder moved since");
    assert!(
        !warning_due(Some((pvs, true)), pvs, (3, 1)),
        "and said once"
    );
    assert!(
        warning_due(Some((pvs, true)), Revision::default(), (1, 0)),
        "a new bake"
    );
}
