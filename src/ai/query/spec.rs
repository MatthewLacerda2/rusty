//! What an environment query asks (#1892): a generator, its tests and how to pick.
//!
//! Every place a spec names (the target, the querier, a list of points) arrives
//! here **resolved**: the caller (the Lua binding) turns context names into
//! [`Anchor`]s and point lists before the query runs, so a bad name is an error
//! at the call naming its field, and the query itself never fails.

use glam::Vec3;

/// The most items a query may generate (#1892, decision 5): a bad spec can't
/// stall a tick.
pub const MAX_ITEMS: usize = 512;

/// A resolved context: where it is, which way it faces (an entity's +Z), and the
/// entity it is, when it is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub position: Vec3,
    /// The entity's world forward (+Z); `None` for a bare point.
    pub facing: Option<Vec3>,
    pub entity: Option<u32>,
}

impl Anchor {
    /// Entity `id` as a context: where it **stands** ([`stand::feet`], not its
    /// Transform, which for a character is its middle) and its world forward
    /// (+Z).
    ///
    /// [`stand::feet`]: crate::ai::stand::feet
    pub fn of_entity(scene: &crate::scene::Scene, id: u32) -> Self {
        Self {
            position: crate::ai::stand::feet(scene, id),
            facing: Some(scene.world.world_rotation(id) * Vec3::Z),
            entity: Some(id),
        }
    }

    /// A bare point: no facing, no entity.
    pub fn point(position: Vec3) -> Self {
        Self {
            position,
            facing: None,
            entity: None,
        }
    }
}

/// Where candidate items come from. Every generated point is then projected onto
/// the navmesh within [`QuerySpec::project`] (Unreal's *Project* step).
#[derive(Clone, Debug, PartialEq)]
pub enum Generator {
    /// Rings around `center`, Unreal's *Donut* / *OnCircle*: `rings` radii from
    /// `inner` to `outer` (one ring sits midway), `per_ring` points on each,
    /// the first at +X, turning toward +Z.
    Donut {
        center: Vec3,
        inner: f32,
        outer: f32,
        rings: u32,
        per_ring: u32,
    },
    /// A square grid of `spacing` over `±half_size` around `center`, Unreal's
    /// *SimpleGrid*, row by row from -X -Z.
    Grid {
        center: Vec3,
        half_size: f32,
        spacing: f32,
    },
    /// The points the script gives, in its order.
    Points(Vec<Vec3>),
}

impl Generator {
    /// How many points it generates before projection (a donut's rings × points,
    /// a grid's rows × columns), so an oversized spec is refused before any work.
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

/// Points along one side of a grid: `floor(2·half / spacing) + 1`, and 0 for a
/// spacing that isn't positive or a half size below zero.
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

/// What one test measures for an item, its *raw value*.
#[derive(Clone, Debug, PartialEq)]
pub enum Measure {
    /// Metres from `to` to the item; on the XZ plane when `flat`.
    Distance { to: Anchor, flat: bool },
    /// The cosine between `from`'s facing and the direction from it to the item
    /// (on the XZ plane when `flat`): 1 straight ahead, -1 straight behind.
    Dot { from: Anchor, flat: bool },
    /// 1 when whether a ray from `from` (raised `from_height`) to the item
    /// (raised `item_height`) hits nothing on `mask`'s layers is `visible`, else
    /// 0: `visible = false` asks for cover, and scores it.
    Trace {
        from: Anchor,
        from_height: f32,
        item_height: f32,
        mask: Option<u32>,
        visible: bool,
    },
    /// 1 when whether the item (raised `item_height`) is inside the active
    /// camera's view, its frustum widened by `margin` degrees each side, is
    /// `inside`, else 0.
    View {
        margin: f32,
        item_height: f32,
        inside: bool,
    },
    /// 1 when the querier can walk to the item, else 0.
    PathExists,
    /// Travel cost from the querier to the item along the navmesh, metres; an item
    /// it cannot reach fails the test and is dropped.
    PathLength,
    /// Metres from the item to the nearest other agent's target or position,
    /// capped at `radius` (everything past it is "far enough").
    Separation { radius: f32 },
    /// Metres from the item to the nearest of `points` (infinite with none).
    NearPoints { points: Vec<Vec3> },
}

impl Measure {
    /// How dear the test is, Unreal's test cost: the query runs the cheap tests
    /// first so the dear ones only see their survivors. 0 is pure maths, 1 a ray
    /// per item, 2 a navmesh search.
    pub fn cost(&self) -> u8 {
        match self {
            Self::Trace { .. } => 1,
            Self::PathExists | Self::PathLength => 2,
            _ => 0,
        }
    }

    /// Whether the raw value is a yes/no (1 when the answer is the one the test
    /// asks for, else 0), filtered by [`Filter::Wanted`].
    pub fn is_bool(&self) -> bool {
        matches!(
            self,
            Self::Trace { .. } | Self::View { .. } | Self::PathExists
        )
    }
}

/// How a test drops items. A numeric test keeps raw values within `min..=max`
/// (either end open); a yes/no test keeps the items whose answer is the one its
/// measure asks for (`visible`, `inside`, reachable).
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

crate::core::named_enum! {
    /// How a normalized raw value (0..1) becomes a score before its weight.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Curve {
        /// `n`: more is better.
        #[default]
        Linear = "linear",
        /// `1 - n`: less is better.
        Inverse = "inverse",
        /// `n²`: favours the top end.
        Square = "square",
        /// `√n`: lifts the low end.
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

/// How a test scores: its curve and weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub curve: Curve,
    pub weight: f32,
}

/// One test: what it measures, and whether it filters, scores, or both
/// (Unreal's `TestPurpose`).
#[derive(Clone, Debug, PartialEq)]
pub struct Test {
    pub measure: Measure,
    pub filter: Option<Filter>,
    pub score: Option<Score>,
}

/// A whole query.
#[derive(Clone, Debug, PartialEq)]
pub struct QuerySpec {
    pub generator: Generator,
    /// How far a generated point may be from the walkable floor it projects onto,
    /// metres; farther points are dropped.
    pub project: f32,
    pub tests: Vec<Test>,
}

/// The default projection reach, metres: a point at a context's height finds the
/// floor under a standing character.
pub const DEFAULT_PROJECT: f32 = 2.0;

crate::core::named_enum! {
    /// Which item a query hands back (Unreal's `EEnvQueryRunMode`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RunMode {
        /// The highest score; a tie goes to the earliest-generated item.
        #[default]
        Best = "best",
        /// A seeded random pick among the items scoring in the top 5% of the
        /// score range.
        RandomBest5 = "random_best_5",
        /// The same over the top 25%.
        RandomBest25 = "random_best_25",
    }
}
