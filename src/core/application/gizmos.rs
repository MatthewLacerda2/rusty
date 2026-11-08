//! The Scene view's gizmo set (#1909): which editor gizmos the Scene view draws,
//! Unity's Gizmos menu and `GizmoUtility.SetGizmoEnabled`, and the Game view's one
//! **Gizmos** toggle (`GameGizmos`, #1882), a row like the others. Here, below the editor and
//! the renderer, because both read it: the editor owns the set and shows its
//! toggles, the shell hands it to the render view, the renderer gates its gizmos on
//! it, and `Editor.SetGizmo` names a flag by its row.
//!
//! **One table.** A gizmo is one row of [`gizmos!`]'s list below: its variant, its
//! `Editor.SetGizmo` name, its toggle's label and hover text, whether it starts on
//! (Unity's default) and whether the tab strip shows its toggle yet. A new gizmo adds
//! a row and the draw it gates; nothing else lists the flags.
//!
//! The set is per session, as the `UI` overlay toggle it replaced was: a new editor
//! starts from the rows' defaults.
//!
//! The mailbox half (as `demo`'s): a script's `Editor.SetGizmo` lands in the shared
//! [`Application`] at once (so `Editor.GetGizmo` reads it back on the next line) and
//! waits there for the editor window, which takes it at the frame boundary and
//! mirrors its own set back each frame.

use super::Application;

/// One gizmo's row: what the API, the toggle and a fresh editor know of it.
#[derive(Clone, Copy, Debug)]
pub struct GizmoRow {
    /// Its `Editor.SetGizmo` / `Editor.GetGizmo` name.
    pub name: &'static str,
    /// The Scene tab strip toggle's label.
    pub label: &'static str,
    /// The toggle's hover text.
    pub hover: &'static str,
    /// Whether a fresh editor draws it (Unity's default).
    pub on: bool,
    /// Whether the tab strip shows its toggle: `false` while nothing draws it yet,
    /// the flag still set and read through the API.
    pub shown: bool,
    /// Whether its toggle sits on the Game tab's strip, not the Scene tab's: a flag
    /// of what the Game view draws.
    pub game: bool,
}

macro_rules! gizmos {
    ($($variant:ident => $row:expr,)*) => {
        /// A gizmo flag, one per row of the table.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Gizmo {
            $($variant,)*
        }

        impl Gizmo {
            /// Every gizmo, in the tab strip's order.
            pub const ALL: &[Gizmo] = &[$(Gizmo::$variant,)*];
            const ROWS: &[GizmoRow] = &[$($row,)*];
            const COUNT: usize = Self::ROWS.len();
        }
    };
}

// The tab strip's order, left to right; one row per gizmo.
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
        hover: "Draw each reflection probe's capture point and parallax box",
        on: true,
        shown: true,
        game: false,
    },
    // Every collider's shape (#1882); off, as Unity draws only the selection's
    // colliders until asked. The selected object's colliders draw either way.
    Colliders => GizmoRow {
        name: "Colliders",
        label: "Colliders",
        hover: "Draw every collider's shape (the selected object's always draw)",
        on: false,
        shown: true,
        game: false,
    },
    // Hidden until the navmesh gizmo draws something (#1910); off, as Unity's is:
    // it paints the whole floor.
    NavMesh => GizmoRow {
        name: "NavMesh",
        label: "NavMesh",
        hover: "Draw the baked navigation mesh",
        on: false,
        shown: false,
        game: false,
    },
    // The Game tab's one toggle (#1882), Unity's Game-view **Gizmos**: the colliders
    // and the `Debug.Draw*` lines over the game frame. Off, as Unity's starts.
    GameGizmos => GizmoRow {
        name: "GameGizmos",
        label: "Gizmos",
        hover: "Draw the colliders and the Debug.Draw lines over the game",
        on: false,
        shown: true,
        game: true,
    },
}

impl Gizmo {
    /// This gizmo's row.
    pub fn row(self) -> &'static GizmoRow {
        &Self::ROWS[self as usize]
    }

    /// The gizmo `Editor.SetGizmo` names `name` (exact, case-sensitive).
    pub fn named(name: &str) -> Option<Gizmo> {
        Self::ALL.iter().copied().find(|g| g.row().name == name)
    }
}

/// Which gizmos the Scene view draws. `Default` is every row's `on`.
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
    /// Whether `gizmo` is drawn.
    pub fn on(&self, gizmo: Gizmo) -> bool {
        self.on[gizmo as usize]
    }

    /// Draw `gizmo` or stop drawing it.
    pub fn set(&mut self, gizmo: Gizmo, on: bool) {
        self.on[gizmo as usize] = on;
    }

    /// `gizmo`'s flag, to bind to a toggle.
    pub fn flag_mut(&mut self, gizmo: Gizmo) -> &mut bool {
        &mut self.on[gizmo as usize]
    }
}

/// The mailbox: a script's flag changes in, the editor's set mirrored out.
#[derive(Default)]
pub struct GizmoDesk {
    viewer: bool,
    set: SceneGizmos,
    requests: Vec<(Gizmo, bool)>,
}

impl Application {
    /// Mark the host as one with a Scene view (the editor window).
    pub fn mark_gizmo_viewer(&mut self) {
        self.gizmos.viewer = true;
    }

    /// Whether the host has a Scene view whose gizmos a script may set.
    pub fn gizmo_viewer(&self) -> bool {
        self.gizmos.viewer
    }

    /// The Scene view's gizmo set, as the editor last mirrored it and scripts set it.
    pub fn scene_gizmos(&self) -> SceneGizmos {
        self.gizmos.set
    }

    /// A script sets `gizmo`: read back at once, taken by the editor at the next frame.
    pub fn request_gizmo(&mut self, gizmo: Gizmo, on: bool) {
        self.gizmos.set.set(gizmo, on);
        self.gizmos.requests.push((gizmo, on));
    }

    /// The editor's frame: apply the scripts' requests to its `set`, oldest first,
    /// and mirror the result here.
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
