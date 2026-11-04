//! A stochastic op's own `seed` (#1487), for every catalog row with a `seed` param:
//! each recipe is built from the catalog, so a new seeded op is checked with
//! nothing added here.

use serde_json::{json, Value};

use super::test_graph::{bake, bits};
use crate::procgen::recipe::{sample_op, OpInfo, OPS};

/// The catalog rows with a `seed` param.
fn seeded() -> impl Iterator<Item = &'static OpInfo> {
    OPS.iter()
        .filter(|i| i.params.iter().any(|p| p.name == "seed"))
}

/// `info`'s sample op (required params filled, the rest at their defaults) with
/// `set` applied, baked with its inputs wired.
fn bake_with(info: &OpInfo, set: &[(&str, Value)]) -> Vec<u32> {
    let mut op = serde_json::to_value(sample_op(info)).unwrap();
    op.as_object_mut().unwrap().remove("seed"); // written only when `set` names it
    for (name, value) in set {
        op[*name] = value.clone();
    }
    let wired = sample_op(info).arity().max as usize;
    bits(&bake(op, wired, Vec::new(), [32, 32]))
}

/// The settings to try a seed under: the defaults, then each enum param at each of
/// its values and each optional number moved off its default. A seed may matter
/// only under one of them (`brick`'s `output = "random"`, `tile`'s `jitter`).
fn settings(info: &OpInfo) -> Vec<Vec<(&'static str, Value)>> {
    let mut out = vec![Vec::new()];
    for p in info.params.iter().filter(|p| p.name != "seed") {
        match p.ty {
            "enum" => out.extend(p.values.iter().map(|v| vec![(p.name, json!(v))])),
            "number" if p.default.is_some() => out.push(vec![(p.name, json!(0.4))]),
            _ => {}
        }
    }
    out
}

#[test]
fn every_stochastic_op_changes_with_its_seed() {
    for info in seeded() {
        let changes = settings(info).into_iter().any(|set| {
            let mut reseeded = set.clone();
            reseeded.push(("seed", json!(5)));
            bake_with(info, &set) != bake_with(info, &reseeded)
        });
        assert!(
            changes,
            "{}: no setting's bake changes with its seed",
            info.op
        );
    }
    assert!(seeded().count() > 0, "the catalog has seeded ops to check");
}

#[test]
fn a_zero_seed_bakes_what_no_seed_does() {
    // 0 keeps the recipe seed, so a recipe written before #1487 bakes as it did.
    for info in seeded() {
        for set in settings(info) {
            let mut zero = set.clone();
            zero.push(("seed", json!(0)));
            assert_eq!(bake_with(info, &set), bake_with(info, &zero), "{}", info.op);
        }
    }
}
