use glam::Vec3;

/// Points a generator may make; the Lua binding refuses more, so no spec stalls
/// a tick.
pub const MAX_ITEMS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub position: Vec3,
    pub facing: Option<Vec3>,
    pub entity: Option<u32>,
}

impl Anchor {
    pub fn of_entity(scene: &rusty_scene::Scene, id: u32) -> Self {
        Self {
            position: crate::stand::feet(scene, id),
            facing: Some(scene.world.world_rotation(id) * Vec3::Z),
            entity: Some(id),
        }
    }

    pub fn point(position: Vec3) -> Self {
        Self {
            position,
            facing: None,
            entity: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Generator {
    Donut {
        center: Vec3,
        inner: f32,
        outer: f32,
        rings: u32,
        per_ring: u32,
    },
    Grid {
        center: Vec3,
        half_size: f32,
        spacing: f32,
    },
    Points(Vec<Vec3>),
}

impl Generator {
    pub fn count(&self) -> usize {
        match self {
            Self::Donut {
                rings, per_ring, ..
            } => (*rings as usize).saturating_mul(*per_ring as usize),
            Self::Grid {
                half_size, spacing, ..
            } => {
                let side = grid_side(*half_size, *spacing);
                side.saturating_mul(side)
            }
            Self::Points(p) => p.len(),
        }
    }
}

pub(super) fn grid_side(half_size: f32, spacing: f32) -> usize {
    if spacing.is_nan() || half_size.is_nan() || spacing <= 0.0 || half_size < 0.0 {
        return 0;
    }
    let side = (2.0 * half_size / spacing).floor() + 1.0;
    if side.is_finite() {
        side.min(usize::MAX as f32) as usize
    } else {
        usize::MAX
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Measure {
    /// Metres from `to` to the item; on the XZ plane when `flat`.
    Distance { to: Anchor, flat: bool },
    /// 1 straight ahead, -1 straight behind.
    Dot { from: Anchor, flat: bool },
    /// 1 when the ray's being clear equals `visible`: `false` asks for cover.
    Trace {
        from: Anchor,
        from_height: f32,
        item_height: f32,
        mask: Option<u32>,
        visible: bool,
    },
    /// 1 when being in view equals `inside`, else 0.
    View {
        margin: f32,
        item_height: f32,
        inside: bool,
    },
    /// 1 when walkable, else 0.
    PathExists,
    /// Navmesh metres; unreachable is dropped.
    PathLength,
    /// Metres to the nearest other agent's target or position, capped at `radius`.
    Separation { radius: f32 },
    /// Metres from the item to the nearest of `points` (infinite with none).
    NearPoints { points: Vec<Vec3> },
}

impl Measure {
    pub fn cost(&self) -> u8 {
        match self {
            Self::Trace { .. } => 1,
            Self::PathExists | Self::PathLength => 2,
            _ => 0,
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(
            self,
            Self::Trace { .. } | Self::View { .. } | Self::PathExists
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    Range { min: Option<f32>, max: Option<f32> },
    Wanted,
}

impl Filter {
    pub fn keeps(self, raw: f32) -> bool {
        match self {
            Self::Range { min, max } => {
                min.is_none_or(|m| raw >= m) && max.is_none_or(|m| raw <= m)
            }
            Self::Wanted => raw > 0.5,
        }
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Curve {
        #[default]
        Linear = "linear",
        Inverse = "inverse",
        Square = "square",
        Sqrt = "sqrt",
    }
}

impl Curve {
    pub fn shape(self, n: f32) -> f32 {
        match self {
            Self::Linear => n,
            Self::Inverse => 1.0 - n,
            Self::Square => n * n,
            Self::Sqrt => n.sqrt(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub curve: Curve,
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Test {
    pub measure: Measure,
    pub filter: Option<Filter>,
    pub score: Option<Score>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuerySpec {
    pub generator: Generator,
    pub project: f32,
    pub tests: Vec<Test>,
}

/// Metres a generated point may sit off its floor, by default.
pub const DEFAULT_PROJECT: f32 = 2.0;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RunMode {
        #[default]
        Best = "best",
        RandomBest5 = "random_best_5",
        RandomBest25 = "random_best_25",
    }
}

impl QuerySpec {
    pub fn shift_origin(&mut self, by: Vec3) {
        match &mut self.generator {
            Generator::Donut { center, .. } | Generator::Grid { center, .. } => *center += by,
            Generator::Points(points) => points.iter_mut().for_each(|p| *p += by),
        }
        for test in &mut self.tests {
            match &mut test.measure {
                Measure::Distance { to: a, .. }
                | Measure::Dot { from: a, .. }
                | Measure::Trace { from: a, .. } => a.position += by,
                Measure::NearPoints { points } => points.iter_mut().for_each(|p| *p += by),
                _ => {}
            }
        }
    }
}
