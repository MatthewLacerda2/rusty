use serde_json::{json, Value};

use super::maps::{maps_fail, seam};
use super::{bake, doc, OUTPUTS, QUICK, RES};

/// `(material, rule, why)`: excused by name with its reason, never by loosening a rule for all.
const EXCUSED: &[(&str, &str, &str)] = &[];

fn excused(path: &str, rule: &str) -> bool {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".graph.json");
    EXCUSED.iter().any(|(m, r, _)| *m == stem && *r == rule)
}

fn interface(d: &Value) -> (Vec<&'static str>, Vec<(&'static str, String)>) {
    let mut found = Vec::new();
    let wear = &d["params"]["wear"];
    if *wear != json!({ "type": "number", "default": 0.3, "min": 0, "max": 1 }) {
        let what = format!("`wear` must be a number 0–1, default 0.3: {wear}");
        found.push(("interface", what));
    }
    if !d["params"]["tint"].is_null() && d["params"]["tint"]["type"] != "color" {
        found.push(("interface", "`tint` must be a color".into()));
    }
    let mut names: Vec<&str> = OUTPUTS.to_vec();
    if d["outputs"].get("metallic").is_some() {
        names.push("metallic");
    }
    for name in &names {
        if d["outputs"].get(name).is_none() {
            found.push(("interface", format!("no `{name}` output")));
        }
    }
    (names, found)
}

pub fn material(path: &str) -> Vec<String> {
    let (names, mut found) = interface(&doc(path));
    if !found.is_empty() {
        return report(path, found);
    }
    let mut fail = |rule: &'static str, what: String| found.push((rule, what));
    let set = |wear: f32, seed: u64| bake(path, json!({ "wear": wear }), seed, &names, RES);
    // What only compares two bakes runs at the small size: equal or not, any size tells.
    let quick = |wear: f32, seed: u64| bake(path, json!({ "wear": wear }), seed, &names, QUICK);
    let first = quick(0.3, 0);
    if first != quick(0.3, 0) {
        fail("deterministic", "two bakes at wear 0.3 differ".into());
    }
    if first == quick(0.3, 7) {
        fail("seed", "the instance seed changes nothing".into());
    }
    let (new, typical, derelict) = (set(0.0, 0), set(0.3, 0), set(1.0, 0));
    if new == derelict {
        fail("wear", "wear 0 and wear 1 bake alike".into());
    }
    for (wear, maps) in [(0.0, &new), (0.3, &typical), (1.0, &derelict)] {
        for (name, img) in names.iter().zip(maps.iter()) {
            if let Some(seam) = seam(img) {
                fail("tiles", format!("{name} at wear {wear}: {seam}"));
            }
        }
        let metal = names
            .iter()
            .position(|n| *n == "metallic")
            .map(|i| &maps[i]);
        for (rule, what) in maps_fail(maps, metal) {
            fail(rule, format!("at wear {wear}: {what}"));
        }
    }
    report(path, found)
}

fn report(path: &str, found: Vec<(&str, String)>) -> Vec<String> {
    let kept = found.into_iter().filter(|(rule, _)| !excused(path, rule));
    kept.map(|(rule, what)| format!("{path}: [{rule}] {what}"))
        .collect()
}
