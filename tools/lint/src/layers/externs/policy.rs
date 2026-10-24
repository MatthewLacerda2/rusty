//! What the crate half of the table needs beyond one row per module.

/// A crate only files under one of `within` may name (paths from the repo root).
pub struct Confined {
    pub krate: &'static str,
    pub within: &'static [&'static str],
}

/// A crate use the table's rules forbid but the code still has, tracked by an open
/// issue. Allowed until fixed; reported as stale the moment it disappears.
pub struct CrateException {
    pub module: &'static str,
    pub krate: &'static str,
    pub issue: u32,
}

pub struct Policy {
    /// The vocabulary every module may name without its row listing it.
    pub common: &'static [&'static str],
    /// Dependencies no module names: each says why it is in `Cargo.toml` at all.
    pub unscoped: &'static [(&'static str, &'static str)],
    pub confined: &'static [Confined],
    pub exceptions: &'static [CrateException],
}

pub const POLICY: Policy = Policy {
    // The math, the scene document's serde and logging: CLAUDE.md's "use glam for
    // all math", and every module saves or logs something.
    common: &["glam", "log", "serde", "serde_json"],
    unscoped: &[],
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
    exceptions: &[],
};

const fn egui(krate: &'static str) -> Confined {
    Confined {
        krate,
        within: &["src/editor", "src/shell/editor"],
    }
}
