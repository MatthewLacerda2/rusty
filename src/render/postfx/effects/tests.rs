//! The registry's own shape (#1467): every effect's module says the import path
//! its file does, the assembled composite and bake compose with no GPU, slots never
//! overlap, and a finishing LDR pass runs last.

use super::assemble::{bake_source, composite_source, import_path, BAKE, COMPOSITE};
use super::*;
use crate::render::gpu::shaders::ShaderRegistry;

#[test]
fn every_module_names_itself_by_its_path() {
    for module in POST_EFFECTS.iter().filter_map(|e| e.module) {
        let source = std::fs::read_to_string(format!("engine/shaders/{module}")).unwrap();
        let directive = format!("#define_import_path {}", import_path(module));
        assert!(source.contains(&directive), "{module} lacks `{directive}`");
    }
}

#[test]
fn an_effect_with_functions_has_a_module_and_one_without_has_none() {
    for e in POST_EFFECTS {
        let functions = !e.composite.is_empty() || !e.grade.is_empty();
        assert_eq!(functions, e.module.is_some(), "{}", e.name);
    }
}

#[test]
fn the_assembled_composite_and_bake_compose_without_a_gpu() {
    let mut registry = ShaderRegistry::new("engine/shaders");
    assemble::register_modules(&mut registry);
    let composite = registry.compose_source(&composite_source(), COMPOSITE);
    let entries: Vec<&str> = composite
        .entry_points
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(entries, ["vs_fullscreen", "fs_composite"]);
    let bake = registry.compose_source(&bake_source(), BAKE);
    assert_eq!(bake.entry_points[0].name, "cs_bake");
}

#[test]
fn every_function_has_its_own_slots_in_stage_order() {
    let composite = composite_layout();
    for pair in composite.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let key = |p: &Placed<Composite>| (p.function.phase, p.function.order);
        assert!(
            key(a) < key(b),
            "{} and {} share a place",
            a.effect.name,
            b.effect.name
        );
        assert_eq!(
            a.offset + a.function.slots,
            b.offset,
            "slots are packed in order"
        );
    }
    let grade = grade_layout();
    for pair in grade.windows(2) {
        assert!(
            pair[0].function.order < pair[1].function.order,
            "grade orders are unique"
        );
        assert_eq!(pair[0].offset + pair[0].function.slots, pair[1].offset);
    }
    assert_eq!(
        composite_slots_len(),
        composite_slots(&EffectCx {
            post: Default::default(),
            graphics: Default::default(),
            full: true,
        })
        .len()
    );
}

#[test]
fn scene_passes_have_their_own_order_and_a_finishing_pass_runs_last() {
    for ldr in [false, true] {
        let mut orders: Vec<u16> = POST_EFFECTS
            .iter()
            .filter_map(|e| match e.pass {
                Some(Pass::Scene { order, .. }) if !ldr => Some(order),
                Some(Pass::Ldr { order, .. }) if ldr => Some(order),
                _ => None,
            })
            .collect();
        let all = orders.len();
        orders.sort_unstable();
        orders.dedup();
        assert_eq!(orders.len(), all, "two passes of one kind share an order");
    }
    let ldr = ldr_passes();
    let finishing = ldr.iter().position(|p| {
        matches!(
            p,
            Pass::Ldr {
                run: LdrRun::Finish(_),
                ..
            }
        )
    });
    if let Some(at) = finishing {
        assert_eq!(at, ldr.len() - 1, "the finishing pass runs last");
    }
}
