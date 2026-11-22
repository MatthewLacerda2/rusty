pub struct Layer {
    pub module: &'static str,
    pub sim: bool,
    pub deps: &'static [&'static str],
    pub crates: &'static [&'static str],
}

impl Layer {
    const fn uses(self, crates: &'static [&'static str]) -> Self {
        Layer { crates, ..self }
    }
}

const fn sim(module: &'static str, deps: &'static [&'static str]) -> Layer {
    Layer {
        module,
        sim: true,
        deps,
        crates: &[],
    }
}

const fn platform(module: &'static str, deps: &'static [&'static str]) -> Layer {
    Layer {
        module,
        sim: false,
        deps,
        crates: &[],
    }
}

pub const LAYERS: &[Layer] = &[
    sim("core", &[]).uses(&["rustc_hash", "xxhash_rust"]),
    sim("time", &[]),
    sim("font", &["core"]).uses(&["harfrust", "skrifa"]),
    sim("asset", &["core"]).uses(&[
        "bevy_mikktspace",
        "block_compression",
        "bytemuck",
        "gltf",
        "image",
        "ktx2",
        "meshopt",
        "symphonia",
        "tobj",
        "ufbx",
    ]),
    sim("procgen", &["core", "font"]).uses(&["image"]),
    sim("shadergen", &["core"]).uses(&["naga", "naga_oil"]),
    sim("components", &["asset", "core", "procgen"]),
    sim("ecs", &["components", "core"]).uses(&["hecs"]),
    sim("audio", &["asset", "components", "core"]).uses(&["kira"]),
    sim("scene", &["asset", "components", "core", "ecs"]),
    sim(
        "edit",
        &[
            "asset",
            "components",
            "core",
            "ecs",
            "procgen",
            "scene",
            "shadergen",
        ],
    ),
    // The lightmap bake traces rays through parry (via `rapier3d`); `physics` sits above.
    sim(
        "lighting",
        &["asset", "components", "core", "procgen", "scene"],
    )
    .uses(&["rapier3d"]),
    // Sim: the in-game UI's layout runs headless, inside the fixed tick.
    sim(
        "ui",
        &["asset", "components", "core", "ecs", "font", "scene"],
    )
    .uses(&["image"]),
    sim("physics", &["components", "core", "ecs", "scene", "time"]).uses(&["rapier3d"]),
    sim(
        "navigation",
        &["components", "core", "ecs", "physics", "scene"],
    ),
    sim(
        "ai",
        &["components", "core", "navigation", "physics", "scene"],
    ),
    sim("zimmer", &["core"]),
    // `scripting` and `api` name each other, so they are one module.
    sim(
        "script",
        &[
            "ai",
            "asset",
            "audio",
            "components",
            "core",
            "ecs",
            "edit",
            "lighting",
            "navigation",
            "physics",
            "procgen",
            "scene",
            "shadergen",
            "time",
            "ui",
            "zimmer",
        ],
    )
    .uses(&["mlua"]),
    sim(
        "app",
        &[
            "ai",
            "asset",
            "audio",
            "components",
            "core",
            "edit",
            "navigation",
            "physics",
            "scene",
            "script",
            "time",
            "ui",
        ],
    ),
    platform("logging", &["core", "script"]).uses(&["env_logger"]),
    platform("preview", &["asset", "components", "core", "edit", "scene"]),
    platform(
        "render",
        &[
            "asset",
            "components",
            "core",
            "ecs",
            "font",
            "physics",
            "scene",
            "shadergen",
            "ui",
        ],
    )
    .uses(&[
        "bytemuck",
        "image",
        "ktx2",
        "naga_oil",
        "objc2_metal",
        "pollster",
        "wgpu",
    ]),
    // The dev-only Lua namespaces (`Debug.*`, the bake verbs) bind here: `script` is sim
    // and may not name a platform crate.
    platform(
        "dev",
        &[
            "app",
            "asset",
            "components",
            "core",
            "edit",
            "lighting",
            "logging",
            "navigation",
            "physics",
            "preview",
            "render",
            "scene",
            "script",
            "shadergen",
            "time",
        ],
    )
    .uses(&[
        "bytemuck",
        "image",
        "libc",
        "libloading",
        "mach2",
        "mlua",
        "pollster",
    ]),
    platform(
        "editor",
        &[
            "asset",
            "audio",
            "components",
            "core",
            "dev",
            "ecs",
            "edit",
            "lighting",
            "navigation",
            "preview",
            "scene",
            "script",
            "shadergen",
            "ui",
        ],
    )
    .uses(&["egui", "egui_phosphor"]),
    platform(
        "shell",
        &[
            "app",
            "asset",
            "audio",
            "core",
            "dev",
            "ecs",
            "edit",
            "editor",
            "logging",
            "navigation",
            "render",
            "scene",
            "script",
            "time",
        ],
    )
    .uses(&[
        "arboard",
        "egui",
        "egui_wgpu",
        "egui_winit",
        "gilrs",
        "image",
        "notify",
        "objc2",
        "objc2_app_kit",
        "objc2_foundation",
        "pollster",
        "smithay_clipboard",
        "wgpu",
        "winit",
    ]),
];
