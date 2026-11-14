//! What the crate half of the table needs beyond one row per module.

/// A crate only files under one of `within` may name (module paths: `crate::modules`).
pub struct Confined {
    pub krate: &'static str,
    pub within: &'static [&'static str],
}

pub struct Policy {
    /// The vocabulary every module may name without its row listing it.
    pub common: &'static [&'static str],
    /// Dependencies no module names: each says why it is in `Cargo.toml` at all.
    pub unscoped: &'static [(&'static str, &'static str)],
    pub confined: &'static [Confined],
    /// Crates no sim member may depend on, tests included: the display layer's.
    pub display: &'static [&'static str],
}

pub const POLICY: Policy = Policy {
    // The math, the scene document's serde and logging: CLAUDE.md's "use glam for
    // all math", and every module saves or logs something.
    common: &["glam", "log", "serde", "serde_json"],
    unscoped: &[(
        "getrandom",
        "a feature pin closing a build/test split under `dev` (docs/testing.md)",
    )],
    confined: &[
        // The editor is a Cargo feature (#431): egui lives only behind it.
        egui("egui"),
        egui("egui_phosphor"),
        egui("egui_wgpu"),
        egui("egui_winit"),
        // Provider calls are authoring, never gameplay (#383).
        Confined {
            krate: "scorsese_providers",
            within: &["src/dev/providers"],
        },
        Confined {
            krate: "scorsese_core",
            within: &["src/dev/providers"],
        },
    ],
    // The sim runs headless, with no GPU and no UI (#494): the arrow is display →
    // sim, never back. The engine's own display crates are platform rows, which
    // `SIM_LEAK` and the dev-dependency check already keep out of a sim crate.
    display: &[
        "egui",
        "egui_phosphor",
        "egui_wgpu",
        "egui_winit",
        "rusty_editor",
        "rusty_render",
        "wgpu",
    ],
};

const fn egui(krate: &'static str) -> Confined {
    Confined {
        krate,
        within: &["src/editor", "src/shell/editor"],
    }
}
