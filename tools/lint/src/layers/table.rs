//! The declared module layering (#724): one row per workspace member
//! `crates/<module>/`, the readable map of the engine's layers. Cargo holds the
//! edges themselves (a crate cannot name a crate it does not depend on); `make
//! layers` holds this table **equal** to the manifests, so the map is never a
//! second copy that drifts.
//!
//! Rows run bottom-up: each of a row's `deps` is declared **above** it, the
//! topological order the dev-dependency check reads "below" from.
//! `sim` marks the deterministic simulation, which cargo cannot know: everything a
//! sim module depends on must be sim too, and no sim crate names a display crate.
//! The determinism gate lets only the platform rows opt out of clippy's clock ban,
//! reading this set instead of keeping a copy (see [`super::sim_dirs`],
//! [`super::platform_roots`]).
//!
//! Each row also names the external crates its manifest depends on (#938), beyond
//! the common ones in [`super::externs::policy`], which also confines some crates
//! to a subtree.
//!
//! The table is exact, not a ceiling: a dep nothing references any more is
//! reported as stale, so this file never claims more than the code does.

/// One member crate `crates/<module>/`, the sibling crates it depends on and the
/// external crates it depends on (#938), beyond the common ones in
/// [`super::externs::policy`].
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
    sim("core", &[]).uses(&["rustc_hash", "xxhash_rust"]),
    sim("time", &[]),
    // Decoding is the asset pipeline's: models, clips through symphonia (#545; only
    // `audio` plays them) and colour lookups through `image` (#1441); textures are
    // block-compressed at import (#1574), and baked lightmap pages
    // are stored as KTX2 (#1694); big meshes are split into clusters (#1968).
    sim("asset", &["core"]).uses(&[
        "bevy_mikktspace",
        "block_compression",
        "gltf",
        "image",
        "ktx2",
        "meshopt",
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
    sim("ui", &["asset", "components", "core", "ecs", "scene"])
        .uses(&["harfrust", "image", "skrifa"]),
    sim("physics", &["components", "core", "ecs", "scene", "time"]).uses(&["rapier3d"]),
    sim(
        "navigation",
        &["components", "core", "ecs", "physics", "scene"],
    ),
    // Agents' decisions over the world (#1892): the environment query reads the
    // navmesh, rays through physics and the active camera. Unreal's `AIModule`.
    sim(
        "ai",
        &["components", "core", "navigation", "physics", "scene"],
    ),
    // The Lua side: `scripting` (the VM, lifecycle, timers, console) and `api` (the
    // one surface) name each other, so they are one module, two halves of one crate.
    sim(
        "script",
        &[
            "ai",
            "asset",
            "audio",
            "components",
            "core",
            "ecs",
            "navigation",
            "physics",
            "procgen",
            "scene",
            "shadergen",
            "time",
            "ui",
        ],
    )
    .uses(&["mlua", "zimmer"]),
    sim(
        "app",
        &[
            "ai",
            "asset",
            "audio",
            "components",
            "core",
            "navigation",
            "physics",
            "scene",
            "script",
            "time",
            "ui",
        ],
    ),
    // The platform layer: real time, the GPU, windows and the editor live here.
    // The process logger (#1139): stderr, plus engine warnings for the console.
    platform("logging", &["core", "script"]).uses(&["env_logger"]),
    platform("preview", &["asset", "components", "core", "scene"]),
    platform(
        "render",
        &[
            "asset",
            "components",
            "core",
            "ecs",
            // The collider gizmo draws the shape physics collides with (#1882).
            "physics",
            "scene",
            "shadergen",
            "ui",
        ],
    )
    .uses(&["bytemuck", "image", "ktx2", "naga_oil", "pollster", "wgpu"]),
    // The dev-only Lua namespaces (`Debug.*`, the provider ones) bind here because
    // `api` is sim and may not import `dev` (#737); the harness drives the VM too.
    platform(
        "dev",
        &[
            "app",
            "asset",
            "components",
            "core",
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
        "libloading",
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
            "script",
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
