pub const SCRIPTS: &[(&str, &str)] = &[
    (
        "bot.lua",
        include_str!("../../../../engine/scripts/bot.lua"),
    ),
    (
        "controller_push.lua",
        include_str!("../../../../engine/scripts/controller_push.lua"),
    ),
    (
        "player_controller.lua",
        include_str!("../../../../engine/scripts/player_controller.lua"),
    ),
];

pub const UI_SCRIPTS: &[(&str, &str)] = &[
    (
        "button.lua",
        include_str!("../../../../engine/scripts/ui/button.lua"),
    ),
    (
        "dropdown.lua",
        include_str!("../../../../engine/scripts/ui/dropdown.lua"),
    ),
    (
        "dropdown_item.lua",
        include_str!("../../../../engine/scripts/ui/dropdown_item.lua"),
    ),
    (
        "input_field.lua",
        include_str!("../../../../engine/scripts/ui/input_field.lua"),
    ),
    (
        "range_axis.lua",
        include_str!("../../../../engine/scripts/ui/range_axis.lua"),
    ),
    (
        "scroll_view.lua",
        include_str!("../../../../engine/scripts/ui/scroll_view.lua"),
    ),
    (
        "scrollbar.lua",
        include_str!("../../../../engine/scripts/ui/scrollbar.lua"),
    ),
    (
        "slider.lua",
        include_str!("../../../../engine/scripts/ui/slider.lua"),
    ),
    (
        "toggle.lua",
        include_str!("../../../../engine/scripts/ui/toggle.lua"),
    ),
    (
        "toggle_group.lua",
        include_str!("../../../../engine/scripts/ui/toggle_group.lua"),
    ),
];

pub const STARTER_MATERIALS: &[u8] = include_bytes!("../../../../engine/materials/starter.gltf");
pub const STARTER_MATERIALS_PATH: &str = "assets/materials/starter.gltf";

pub fn script(rel: &str) -> Option<&'static str> {
    let (list, name) = match rel.strip_prefix("ui/") {
        Some(name) => (UI_SCRIPTS, name),
        None => (SCRIPTS, rel),
    };
    list.iter().find(|(n, _)| *n == name).map(|(_, code)| *code)
}
