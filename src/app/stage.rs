//! src/app/stage.rs — Stage definitions
//!
//! The Unity-style execution order. `Startup` runs once when a Play session
//! begins; the four per-frame stages then run every tick in this order:
//! `FixedUpdate → Update → LateUpdate → Render`. Order *within* a stage is the
//! order modules `register` their systems (see `app::schedule`).
//!
//! `FixedUpdate` is the deterministic, fixed-dt stage the headless harness steps;
//! the simulation systems live there so a fixed-timestep replay is reproducible.
//!
//! Allowed deps: none.

/// One slot in the schedule. The ordering of the variants here is the canonical
/// execution order, materialised by [`Stage::FRAME_ORDER`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Runs once when a Play session begins (mirrors Unity's `Start`).
    Startup,
    /// Deterministic fixed-dt simulation (physics, nav, scripts, animation).
    FixedUpdate,
    /// General per-frame logic.
    Update,
    /// Per-frame logic that must run after `Update` (cameras, follow-up passes).
    LateUpdate,
    /// Per-frame draw-data preparation (no GPU work — that stays in `render`).
    Render,
}

impl Stage {
    /// The per-frame stages in execution order. `Startup` is intentionally absent:
    /// it is run on its own when a Play session starts, not every frame.
    pub const FRAME_ORDER: [Stage; 4] = [
        Stage::FixedUpdate,
        Stage::Update,
        Stage::LateUpdate,
        Stage::Render,
    ];

    /// Index into a fixed-size per-stage table. Keeps `Schedule` allocation-free.
    pub(crate) const fn index(self) -> usize {
        match self {
            Stage::Startup => 0,
            Stage::FixedUpdate => 1,
            Stage::Update => 2,
            Stage::LateUpdate => 3,
            Stage::Render => 4,
        }
    }

    /// Total number of stages, for sizing the per-stage table.
    pub(crate) const COUNT: usize = 5;
}
