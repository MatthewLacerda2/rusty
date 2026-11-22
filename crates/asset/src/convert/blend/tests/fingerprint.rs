#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn converter_fingerprints_hold_for_blender() {
    crate::convert::fingerprint::gate(|c| c.tool.is_some());
}
