//! Each test kind's own table (#1892): `distance = { to = "target" }` and the
//! rest, read into a [`Measure`] and whether it is a yes/no test (its answer
//! asked for inside the measure: `visible`, `inside`).

use mlua::Table;

use super::contexts::{Contexts, QUERIER};
use super::fields::{at, fail, number, only, text};
use super::spec::{place, point_list, wanted};
use rusty_ai::query::Measure;

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
) -> mlua::Result<(Measure, bool)> {
    Ok(match kind {
        "distance" => {
            only(t, path, &["to", "mode"])?;
            let to = place(t, path, "to", QUERIER, contexts)?;
            (
                Measure::Distance {
                    to,
                    flat: flat(t, path)?,
                },
                false,
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
                false,
            )
        }
        "trace" => trace(t, path, contexts)?,
        "view" => (view(t, path)?, true),
        "separation" => {
            only(t, path, &["radius"])?;
            let radius = number(t, path, "radius", Some(SEPARATION_RADIUS))?;
            if radius <= 0.0 {
                return fail(&at(path, "radius"), "must be above 0");
            }
            (Measure::Separation { radius }, false)
        }
        "near_points" => {
            only(t, path, &["points"])?;
            let points = point_list(t, path, "points", contexts)?;
            (Measure::NearPoints { points }, false)
        }
        other => return fail(path, format!("unknown test '{other}'")),
    })
}

/// `view = { inside, margin, height }`.
fn view(t: &Table, path: &str) -> mlua::Result<Measure> {
    only(t, path, &["inside", "margin", "height"])?;
    Ok(Measure::View {
        margin: number(t, path, "margin", Some(0.0))?,
        item_height: number(t, path, "height", Some(EYE_HEIGHT))?,
        inside: wanted(t, path, "inside")?,
    })
}

/// `trace = { from, visible, from_height, item_height, mask }`.
fn trace(t: &Table, path: &str, contexts: &Contexts) -> mlua::Result<(Measure, bool)> {
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
        visible: wanted(t, path, "visible")?,
    };
    Ok((measure, true))
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
