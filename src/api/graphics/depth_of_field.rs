//! `Graphics.{Get,Set}DepthOfFieldMode` and one `{Get,Set}DepthOfField<knob>` pair
//! per numeric knob (#1445), on the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls. The knobs
//! come from `DOF_KNOBS`, the table the tween paths read too.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::components::DofMode;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// The mode and every knob. Without an active volume the getters report what a new
/// volume starts with.
pub(super) fn register_depth_of_field<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetDepthOfFieldMode",
        scope.create_function(|_, name: String| {
            let mode = DofMode::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "Graphics.SetDepthOfFieldMode: unknown mode '{name}' (one of {})",
                    DofMode::names()
                ))
            })?;
            with_vc_mut(scene, |e| vc_ops::set_dof_mode(e, mode));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetDepthOfFieldMode",
        scope.create_function(|_, ()| {
            let mode = with_vc(scene, |vc| vc.depth_of_field.mode).unwrap_or_default();
            Ok(mode.name().to_string())
        }),
    )?;
    for knob in vc_ops::DOF_KNOBS {
        put(
            table,
            &format!("Set{}", knob.api),
            scope.create_function(move |_, value: f32| {
                with_vc_mut(scene, |e| (knob.set)(e, value));
                Ok(())
            }),
        )?;
        put(
            table,
            &format!("Get{}", knob.api),
            scope.create_function(move |_, ()| {
                let dof = with_vc(scene, |vc| vc.depth_of_field).unwrap_or_default();
                Ok((knob.get)(&dof))
            }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::DepthOfField;

    #[test]
    fn every_knob_reads_the_field_its_op_writes() {
        let mut vc = crate::scene::authoring::default_post_processing();
        for (i, knob) in vc_ops::DOF_KNOBS.iter().enumerate() {
            // Values inside every knob's range, distinct per knob, rising so the
            // Gaussian end never sits below its start.
            let value = 1.0 + i as f32 * 0.1;
            (knob.set)(&mut vc, value);
            assert_eq!((knob.get)(&vc.depth_of_field), value, "{}", knob.field);
        }
        assert_ne!(vc.depth_of_field, DepthOfField::default());
    }
}
