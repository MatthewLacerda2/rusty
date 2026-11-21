use super::{RenderSwitch, RenderSwitches};
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_scene::lighting::lightmap_set::LightmapSet;

fn without(switch: RenderSwitch) -> RenderSwitches {
    let mut s = RenderSwitches::ON;
    s.set(switch, false);
    s
}

#[test]
fn a_list_is_handed_whole_while_its_switch_is_on_and_empty_once_it_is_off() {
    let items = [1, 2, 3];
    let probes = RenderSwitch::ReflectionProbes;
    assert_eq!(RenderSwitches::ON.listed(probes, &items), &items);
    assert!(without(probes).listed(probes, &items).is_empty());
    let lamps = without(RenderSwitch::ClusteredLights);
    assert_eq!(lamps.listed(probes, &items), &items);
}

#[test]
fn the_lightmaps_switch_takes_every_page_list_and_no_other_switch_does() {
    let mut set = LightmapSet::default();
    set.pages = vec!["c.ktx2".into()];
    set.directions = vec!["d.ktx2".into()];
    set.masks = vec!["m.ktx2".into()];
    let whole = [&set.pages[..], &set.directions[..], &set.masks[..]];
    assert_eq!(RenderSwitches::ON.lightmap_lists(&set), whole);
    let none: [&[String]; 3] = [&[]; 3];
    assert_eq!(without(RenderSwitch::Lightmaps).lightmap_lists(&set), none);
    assert_eq!(without(RenderSwitch::Shadows).lightmap_lists(&set), whole);
}

#[test]
fn shadows_are_cast_only_with_the_switch_on_outside_a_shadowless_capture() {
    assert!(RenderSwitches::ON.casts_shadows(true));
    assert!(
        !RenderSwitches::ON.casts_shadows(false),
        "shadowless capture"
    );
    let off = without(RenderSwitch::Shadows);
    assert!(!off.casts_shadows(true) && !off.casts_shadows(false));
    assert!(without(RenderSwitch::Lightmaps).casts_shadows(true));
}

#[test]
fn the_ssao_and_ssil_switches_turn_their_option_off_and_leave_the_rest() {
    let options = GraphicsOptions::default();
    assert_eq!(RenderSwitches::ON.gate(options), options);
    for (switch, option) in [
        (RenderSwitch::Ssao, GraphicsOption::Ssao),
        (RenderSwitch::Ssil, GraphicsOption::Ssil),
    ] {
        let gated = without(switch).gate(options);
        let mut expected = options;
        expected.set(option, false);
        assert_eq!(gated, expected, "{switch:?}");
    }
    let mut chosen = options;
    chosen.set(GraphicsOption::Ssr, false);
    assert!(!without(RenderSwitch::Ssao)
        .gate(chosen)
        .allows(GraphicsOption::Ssr));
}
