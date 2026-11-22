//! The editor's gizmo flags: Unity's Gizmos menu (`GizmoUtility.SetGizmoEnabled`), plus
//! the Game view's Gizmos and Stats toggles. Per session: a new editor starts from the
//! rows' defaults. A new gizmo adds a row to [`gizmos!`] and the draw it gates.

use super::Application;

#[derive(Clone, Copy, Debug)]
pub struct GizmoRow {
    pub name: &'static str,
    pub label: &'static str,
    pub hover: &'static str,
    pub on: bool,
    /// Whether the tab strip shows its toggle: `false` while nothing draws it yet,
    /// the flag still set and read through the API.
    pub shown: bool,
    /// Its toggle sits on the Game tab's strip, not the Scene tab's.
    pub game: bool,
}

macro_rules! gizmos {
    ($($variant:ident => $row:expr,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Gizmo {
            $($variant,)*
        }

        impl Gizmo {
            pub const ALL: &[Gizmo] = &[$(Gizmo::$variant,)*];
            const ROWS: &[GizmoRow] = &[$($row,)*];
            const COUNT: usize = Self::ROWS.len();
        }
    };
}

// The tab strip's order, left to right.
gizmos! {
    Ui => GizmoRow {
        name: "UI",
        label: "UI",
        hover: "Draw the screen-space canvases and edit them with the rect tool",
        on: true,
        shown: true,
        game: false,
    },
    LightProbes => GizmoRow {
        name: "LightProbes",
        label: "Light Probes",
        hover: "Draw a marker at each light probe, tinted by the light it baked",
        on: true,
        shown: true,
        game: false,
    },
    ReflectionProbes => GizmoRow {
        name: "ReflectionProbes",
        label: "Reflection Probes",
        hover: "Draw each reflection probe's capture point, and the volume of the one open in the inspector",
        on: true,
        shown: true,
        game: false,
    },
    Colliders => GizmoRow {
        name: "Colliders",
        label: "Colliders",
        hover: "Draw every collider's shape (the selected object's always draw)",
        on: false,
        shown: true,
        game: false,
    },
    NavMesh => GizmoRow {
        name: "NavMesh",
        label: "NavMesh",
        hover: "Draw the baked navigation mesh, where agents can walk",
        on: false,
        shown: true,
        game: false,
    },
    GameGizmos => GizmoRow {
        name: "GameGizmos",
        label: "Gizmos",
        hover: "Draw the colliders and the Debug.Draw lines over the game",
        on: false,
        shown: true,
        game: true,
    },
    Stats => GizmoRow {
        name: "Stats",
        label: "Stats",
        hover: "Show the frame's timings and counters over the last second",
        on: false,
        shown: true,
        game: true,
    },
}

impl Gizmo {
    pub fn row(self) -> &'static GizmoRow {
        &Self::ROWS[self as usize]
    }

    pub fn named(name: &str) -> Option<Gizmo> {
        Self::ALL.iter().copied().find(|g| g.row().name == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneGizmos {
    on: [bool; Gizmo::COUNT],
}

impl Default for SceneGizmos {
    fn default() -> Self {
        let mut on = [false; Gizmo::COUNT];
        for (slot, row) in on.iter_mut().zip(Gizmo::ROWS) {
            *slot = row.on;
        }
        Self { on }
    }
}

impl SceneGizmos {
    pub fn on(&self, gizmo: Gizmo) -> bool {
        self.on[gizmo as usize]
    }

    pub fn set(&mut self, gizmo: Gizmo, on: bool) {
        self.on[gizmo as usize] = on;
    }

    pub fn flag_mut(&mut self, gizmo: Gizmo) -> &mut bool {
        &mut self.on[gizmo as usize]
    }
}

#[derive(Default)]
pub struct GizmoDesk {
    viewer: bool,
    set: SceneGizmos,
    requests: Vec<(Gizmo, bool)>,
}

impl Application {
    pub fn mark_gizmo_viewer(&mut self) {
        self.gizmos.viewer = true;
    }

    pub fn gizmo_viewer(&self) -> bool {
        self.gizmos.viewer
    }

    pub fn scene_gizmos(&self) -> SceneGizmos {
        self.gizmos.set
    }

    pub fn request_gizmo(&mut self, gizmo: Gizmo, on: bool) {
        self.gizmos.set.set(gizmo, on);
        self.gizmos.requests.push((gizmo, on));
    }

    pub fn sync_gizmos(&mut self, set: &mut SceneGizmos) {
        for (gizmo, on) in std::mem::take(&mut self.gizmos.requests) {
            set.set(gizmo, on);
        }
        self.gizmos.set = *set;
    }
}

#[cfg(test)]
#[path = "gizmos_tests.rs"]
mod tests;
