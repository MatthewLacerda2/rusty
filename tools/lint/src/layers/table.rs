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
//! The table is exact, not a ceiling: a dep nothing references any more is
//! reported as stale, so this file never claims more than the code does.

/// One top-level module under `src/` and the modules it may import.
pub struct Layer {
    pub module: &'static str,
    pub sim: bool,
    pub deps: &'static [&'static str],
}

const fn sim(module: &'static str, deps: &'static [&'static str]) -> Layer {
    Layer {
        module,
        sim: true,
        deps,
    }
}

const fn platform(module: &'static str, deps: &'static [&'static str]) -> Layer {
    Layer {
        module,
        sim: false,
        deps,
    }
}

pub const LAYERS: &[Layer] = &[
    // The data the sim runs on.
    sim("core", &[]),
    sim("time", &[]),
    sim("asset", &["core"]),
    sim("procgen", &["core"]),
    // Shader authoring; GPU-free composition lives here so `render` imports it (#722).
    sim("shadergen", &["core"]),
    sim("components", &["asset", "core", "procgen"]),
    sim("ecs", &["components"]),
    // A sim `Resource` stepped by sim time; its device thread never reads a clock.
    sim("audio", &["asset", "components", "core"]),
    sim(
        "scene",
        &["asset", "components", "core", "ecs", "procgen", "shadergen"],
    ),
    // The in-game UI's layout runs headless inside the sim (#417).
    sim("ui", &["components", "core", "ecs", "scene"]),
    sim("physics", &["components", "core", "ecs", "scene", "time"]),
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
    ),
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
    ),
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
    platform("preview", &["asset", "components", "core", "scene"]),
    platform(
        "render",
        &[
            "components",
            "core",
            "ecs",
            "procgen",
            "scene",
            "shadergen",
            "ui",
        ],
    ),
    platform(
        "dev",
        &[
            "api",
            "app",
            "asset",
            "components",
            "core",
            "navigation",
            "preview",
            "render",
            "scene",
            "scripting",
            "ui",
        ],
    ),
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
    ),
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
            "navigation",
            "render",
            "scene",
            "scripting",
            "shadergen",
            "time",
        ],
    ),
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
