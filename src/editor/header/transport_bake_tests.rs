//! Play greyed out while a bake runs (#2042): neither a click nor the shortcut
//! enters Play, and the bake ending gives Play back. Stop is never refused.

use egui_phosphor::regular as icon;

use super::tests::Rig;

#[test]
fn a_running_bake_refuses_play_until_it_ends() {
    let mut rig = Rig::new();
    rig.scene.lighting_bake_running = true;
    rig.frame(vec![]);
    rig.click(icon::PLAY);
    assert!(!rig.playing, "a click is refused");
    rig.shortcut();
    assert!(!rig.playing, "the shortcut is refused");
    rig.scene.lighting_bake_running = false;
    rig.frame(vec![]);
    rig.click(icon::PLAY);
    assert!(rig.playing, "the bake ended: Play is back");
    rig.scene.lighting_bake_running = true;
    rig.click(icon::STOP);
    assert!(!rig.playing, "Stop is never refused");
}
