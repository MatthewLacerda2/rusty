//! Each test kind's own table (#1892): `distance = { to = "target" }` and the
//! rest, read into a [`Measure`] and, for a yes/no test, the answer its filter
//! keeps.

use mlua::Table;

use super::contexts::{Contexts, QUERIER};
use super::fields::{at, fail, number, only, text};
use super::spec::{place, point_list, wanted};
use crate::ai::query::Measure;

/// How high above the floor an eye and a standing body's middle are, metres: a
/// line of sight runs eye to eye by default.
const EYE_HEIGHT: f32 = 1.5;
/// The `separation` radius when none is given, metres.
const SEPARATION_RADIUS: f32 = 5.0;

/// Read the table of test kind `kind` at `path`.
pub fn read(
    t: &Table,
    kind: &str,
    path: &str,
    contexts: &Contexts,
) -> mlua::Result<(Measure, Option<bool>)> {
    Ok(match kind {
        "distance" => {
            only(t, path, &["to", "mode"])?;
            let to = place(t, path, "to", QUERIER, contexts)?;
            (
                Measure::Distance {
                    to,
                    flat: flat(t, path)?,
                },
                None,
            )
        }
        "dot" => {
            only(t, path, &["from", "mode"])?;
            let from = place(t, path, "from", QUERIER, contexts)?;
            if from.facing.is_none() {
                return fail(&at(path, "from"), "a point has no facing; name an entity");
            }
            (
                Measure::Dot {
                    from,
                    flat: flat(t, path)?,
                },
                None,
            )
        }
        "trace" => {
            let keys = ["from", "visible", "from_height", "item_height", "mask"];
            only(t, path, &keys)?;
            let mask = match t.get::<Option<i64>>("mask") {
                Ok(m) => m.map(|m| m as u32),
                Err(_) => return fail(&at(path, "mask"), "expected a layer mask (an integer)"),
            };
            let measure = Measure::Trace {
                from: place(t, path, "from", "target", contexts)?,
                from_height: number(t, path, "from_height", Some(EYE_HEIGHT))?,
                item_height: number(t, path, "item_height", Some(EYE_HEIGHT))?,
                mask,
            };
            (measure, Some(wanted(t, path, "visible")?))
        }
        "view" => {
            only(t, path, &["inside", "margin", "height"])?;
            let measure = Measure::View {
                margin: number(t, path, "margin", Some(0.0))?,
                item_height: number(t, path, "height", Some(EYE_HEIGHT))?,
            };
            (measure, Some(wanted(t, path, "inside")?))
        }
        "separation" => {
            only(t, path, &["radius"])?;
            let radius = number(t, path, "radius", Some(SEPARATION_RADIUS))?;
            if radius <= 0.0 {
                return fail(&at(path, "radius"), "must be above 0");
            }
            (Measure::Separation { radius }, None)
        }
        "near_points" => {
            only(t, path, &["points"])?;
            let points = point_list(t, path, "points", contexts)?;
            (Measure::NearPoints { points }, None)
        }
        other => return fail(path, format!("unknown test '{other}'")),
    })
}

/// `mode = "3d"` (the default) or `"2d"`, which measures on the XZ plane.
fn flat(t: &Table, path: &str) -> mlua::Result<bool> {
    match text(t, path, "mode")?.as_deref() {
        None | Some("3d") => Ok(false),
        Some("2d") => Ok(true),
        Some(other) => fail(
            &at(path, "mode"),
            format!("unknown mode '{other}' (expected 2d, 3d)"),
        ),
    }
}
