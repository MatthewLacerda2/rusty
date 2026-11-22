pub struct Confined {
    pub krate: &'static str,
    pub within: &'static [&'static str],
}

pub struct Policy {
    pub common: &'static [&'static str],
    pub unscoped: &'static [(&'static str, &'static str)],
    pub confined: &'static [Confined],
    /// Crate families no sim member may take, tests included: `egui` bans `egui_extras`.
    pub display: &'static [&'static str],
}

impl Policy {
    pub fn is_display(&self, krate: &str) -> bool {
        self.display.iter().any(|d| {
            krate == *d
                || krate
                    .strip_prefix(d)
                    .is_some_and(|rest| rest.starts_with('_'))
        })
    }
}

pub const POLICY: Policy = Policy {
    common: &["glam", "log", "serde", "serde_json"],
    unscoped: &[],
    confined: &[
        egui("egui"),
        egui("egui_phosphor"),
        egui("egui_wgpu"),
        egui("egui_winit"),
    ],
    display: &["egui", "wgpu"],
};

const fn egui(krate: &'static str) -> Confined {
    Confined {
        krate,
        within: &["src/editor", "src/shell/editor"],
    }
}
