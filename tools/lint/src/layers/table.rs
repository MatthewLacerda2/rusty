//! The declared module layering (#724) — the one place that answers "may `src/x`
//! use `src/y`?". `make layers` checks the source against it.
//!
//! Rows run bottom-up: a module may import only the modules in its `deps`, and each
//! of those must be declared **above** it. That order is what makes the graph
//! acyclic; the only edges that may point down the list are between [`PEERS`].
//! `sim` marks the deterministic simulation; everything a sim module imports must be
//! sim too. The direction guard scans this derived set, and the determinism gate
//! lets only the platform rows opt out of clippy's clock ban, instead of either
//! keeping copies (see [`super::sim_dirs`], [`super::platform_roots`]).
//!
//! Each row also names the external crates its module may use (#938); the rest of
//! that policy (crates every module shares, confined ones) is in
//! [`super::externs::policy`].
//!
//! The table is exact, not a ceiling: a dep nothing references any more is
//! reported as stale, so this file never claims more than the code does.

/// One top-level module under `src/`, the modules it may import and the external
/// crates it may name (#938), beyond the common ones in [`super::externs::policy`].
pub struct Layer {
    pub module: &'static str,
    pub sim: bool,
    pub deps: &'static [&'static str],
    pub crates: &'static [&'static str],
}

impl Layer {
    /// The external crates this module names (`-` written as `_`).
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
    // The data the sim runs on.
    sim("core", &[]).uses(&["rustc_hash"]),
    sim("time", &[]),
    // Decoding is the asset pipeline's: models, clips through symphonia (#545; only
    // `audio` plays them) and colour lookups through `image` (#1441).
    sim("asset", &["core"]).uses(&[
        "bevy_mikktspace",
        "gltf",
        "image",
        "symphonia",
        "tobj",
        "ufbx",
    ]),
    sim("procgen", &["core"]).uses(&["image"]),
    // Shader authoring; GPU-free composition lives here so `render` imports it (#722).
    sim("shadergen", &["core"]).uses(&["naga", "naga_oil"]),
    sim("components", &["asset", "core", "procgen"]).uses(&["bytemuck"]),
    sim("ecs", &["components"]).uses(&["hecs"]),
    // A sim `Resource` stepped by sim time; its device thread never reads a clock.
    sim("audio", &["asset", "components", "core"]).uses(&["kira"]),
    // The lightmap bake traces rays through parry's `TriMesh` (#1330); `physics` sits
    // above `scene`, so the crate is named here rather than the module.
    sim(
        "scene",
        &["asset", "components", "core", "ecs", "procgen", "shadergen"],
    )
    .uses(&["rapier3d"]),
    // The in-game UI's layout runs headless inside the sim (#417).
    sim("ui", &["asset", "components", "core", "ecs", "scene"]).uses(&[
        "ab_glyph",
        "image",
        "ttf_parser",
    ]),
    sim("physics", &["components", "core", "ecs", "scene", "time"]).uses(&["rapier3d"]),
    sim("navigation", &["components", "core", "physics", "scene"]),
    // `scripting` and `api` are one layer (see PEERS): scripting installs the Lua
    // surface, and the surface reads runtime state scripting owns.
    sim(
        "scripting",
        &[
            "api",
            "audio",
            "components",
            "core",
            "navigation",
            "physics",
            "scene",
            "shadergen",
            "time",
            "ui",
        ],
    )
    .uses(&["mlua"]),
    sim(
        "api",
        &[
            "asset",
            "audio",
            "components",
            "core",
            "ecs",
            "navigation",
            "physics",
            "procgen",
            "scene",
            "scripting",
            "shadergen",
            "time",
            "ui",
        ],
    )
    .uses(&["mlua", "zimmer"]),
    sim(
        "app",
        &[
            "asset",
            "audio",
            "components",
            "core",
            "navigation",
            "physics",
            "scene",
            "scripting",
            "time",
            "ui",
        ],
    ),
    // The platform layer: real time, the GPU, windows and the editor live here.
    // The process logger (#1139): stderr, plus engine warnings for the console.
    platform("logging", &["scripting"]).uses(&["env_logger"]),
    platform("preview", &["asset", "components", "core", "scene"]),
    platform(
        "render",
        &[
            "asset",
            "components",
            "core",
            "ecs",
            "scene",
            "shadergen",
            "ui",
        ],
    )
    .uses(&[
        "ab_glyph", "bytemuck", "image", "ktx2", "naga_oil", "pollster", "wgpu",
    ]),
    // The dev-only Lua namespaces (`Debug.*`, the provider ones) bind here because
    // `api` is sim and may not import `dev` (#737); the harness drives the VM too.
    platform(
        "dev",
        &[
            "api",
            "app",
            "asset",
            "components",
            "core",
            "logging",
            "navigation",
            "preview",
            "render",
            "scene",
            "scripting",
        ],
    )
    .uses(&[
        "image",
        "mlua",
        "pollster",
        "scorsese_core",
        "scorsese_providers",
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
            "navigation",
            "preview",
            "scene",
            "scripting",
            "shadergen",
            "ui",
        ],
    )
    .uses(&["egui", "egui_phosphor"]),
    // The window owner: winit, the OS input crates, and wgpu only where it presents
    // frames and hosts the editor's egui-wgpu painter; the editor's file watch (notify).
    platform(
        "shell",
        &[
            "api",
            "app",
            "asset",
            "audio",
            "core",
            "dev",
            "ecs",
            "editor",
            "logging",
            "navigation",
            "render",
            "scene",
            "scripting",
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

/// Pairs declared as one layer: they may reference each other in either order.
/// `scripting` ↔ `api` (#724): untangling them would move `ConsoleLogs`, one of the
/// crate's most-referenced types, for little gain.
pub const PEERS: &[(&str, &str)] = &[("scripting", "api")];

/// An edge the layering forbids but the code still has, each tracked by an open
/// issue. Allowed until fixed; reported as stale the moment it disappears.
pub struct Exception {
    pub from: &'static str,
    pub to: &'static str,
    pub issue: u32,
}

/// Empty since #737 and #738 removed the last two (`api → dev`/`preview`, `dev → editor`).
pub const EXCEPTIONS: &[Exception] = &[];
