//! src/scene/io/bundled.rs — the engine's seed content, embedded (#829).
//!
//! A project can live anywhere, nowhere near the engine checkout, so the files the
//! engine seeds into it travel inside the binary instead of being read from
//! `engine/` at run time. The sources stay in `engine/` (a unit test holds these
//! lists to that folder's contents, so a new script can't be forgotten here).

/// The bundled game scripts (`engine/scripts/*.lua`), by file name.
pub const SCRIPTS: &[(&str, &str)] = &[
    ("bot.lua", include_str!("../../../engine/scripts/bot.lua")),
    (
        "player_controller.lua",
        include_str!("../../../engine/scripts/player_controller.lua"),
    ),
];

/// The UI widget kit's scripts (`engine/scripts/ui/*.lua`, #422), by file name.
pub const UI_SCRIPTS: &[(&str, &str)] = &[
    (
        "button.lua",
        include_str!("../../../engine/scripts/ui/button.lua"),
    ),
    (
        "dropdown.lua",
        include_str!("../../../engine/scripts/ui/dropdown.lua"),
    ),
    (
        "input_field.lua",
        include_str!("../../../engine/scripts/ui/input_field.lua"),
    ),
    (
        "scroll_view.lua",
        include_str!("../../../engine/scripts/ui/scroll_view.lua"),
    ),
    (
        "scrollbar.lua",
        include_str!("../../../engine/scripts/ui/scrollbar.lua"),
    ),
    (
        "slider.lua",
        include_str!("../../../engine/scripts/ui/slider.lua"),
    ),
    (
        "toggle.lua",
        include_str!("../../../engine/scripts/ui/toggle.lua"),
    ),
    (
        "toggle_group.lua",
        include_str!("../../../engine/scripts/ui/toggle_group.lua"),
    ),
];

/// The starter material library (#173): glTF-PBR materials, no textures.
pub const STARTER_MATERIALS: &[u8] = include_bytes!("../../../engine/materials/starter.gltf");
/// Where [`STARTER_MATERIALS`] is seeded, relative to the project root.
pub const STARTER_MATERIALS_PATH: &str = "assets/materials/starter.gltf";
