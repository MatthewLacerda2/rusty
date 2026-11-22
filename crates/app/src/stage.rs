#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Runs once when a Play session begins (mirrors Unity's `Start`).
    Startup,
    /// Deterministic fixed-dt simulation.
    FixedUpdate,
    Update,
    /// Runs after `Update`.
    LateUpdate,
    /// Draw-data preparation only; no GPU work.
    Render,
}

impl Stage {
    /// Per-frame stages in execution order; `Startup` runs on its own.
    pub const FRAME_ORDER: [Stage; 4] = [
        Stage::FixedUpdate,
        Stage::Update,
        Stage::LateUpdate,
        Stage::Render,
    ];

    pub(crate) const fn index(self) -> usize {
        match self {
            Stage::Startup => 0,
            Stage::FixedUpdate => 1,
            Stage::Update => 2,
            Stage::LateUpdate => 3,
            Stage::Render => 4,
        }
    }

    pub(crate) const COUNT: usize = 5;
}
