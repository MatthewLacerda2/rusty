//! src/scene/physics_material/tests.rs — the asset file, the cache and what a
//! collider resolves to (#1878).

use super::*;
use crate::components::CombineMode;

fn path_in(name: &str) -> String {
    crate::test_temp::dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn with_asset(path: Option<&str>) -> ColliderComponent {
    ColliderComponent {
        material: PhysicsMaterial::new(0.9, 0.9),
        material_asset: path.map(str::to_string),
        ..crate::scene::authoring::default_collider()
    }
}

#[test]
fn the_cache_follows_rebakes() {
    use crate::core::fs::written::contract::{follows_rebakes, Bake};
    let friction = |b: Bake| if b == Bake::A { 0.1 } else { 0.7 };
    follows_rebakes(
        &path_in("rebaked.physmat"),
        |p, b| {
            write(p, PhysicsMaterial::new(friction(b), 0.0)).unwrap();
        },
        lookup,
        |m, b| m.is_some_and(|m| m.friction == friction(b)),
    );
}

#[test]
fn a_file_missing_fields_or_out_of_range_reads_clamped_defaults() {
    let m = parse(r#"{"friction": -3, "bounciness": 4, "bounce_combine": "Maximum"}"#).unwrap();
    assert_eq!((m.friction, m.bounciness), (0.0, 1.0));
    assert_eq!(m.bounce_combine, CombineMode::Maximum);
    assert_eq!(m.friction_combine, CombineMode::Average);
    assert_eq!(parse("{}").unwrap(), PhysicsMaterial::default());
}

#[test]
fn a_malformed_file_is_an_error_and_its_lookup_a_miss() {
    for text in ["", "{\"friction\": ", "{\"friction\": 1e999}", "\"wood\""] {
        assert!(parse(text).is_err(), "{text:?} must not parse");
    }
    let path = path_in("torn.physmat");
    std::fs::write(&path, "{\"friction\": ").unwrap();
    assert_eq!(lookup(&path), None);
}

#[test]
fn a_write_round_trips_and_refuses_another_extension() {
    let path = path_in("deep/folder/wood.physmat");
    let wood = PhysicsMaterial {
        friction_combine: CombineMode::Minimum,
        ..PhysicsMaterial::new(0.6, 0.2)
    };
    write(&path, wood).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(parse(&text).unwrap(), wood);
    assert!(write(&path_in("wood.json"), wood).is_err());
    let bad = PhysicsMaterial {
        friction: f32::NAN,
        ..wood
    };
    assert!(write(&path, bad).is_err(), "a NaN is refused");
    assert_eq!(lookup(&path), Some(wood), "a refused write keeps the asset");
}

#[test]
fn a_collider_resolves_inline_asset_or_default() {
    let path = path_in("concrete.physmat");
    let concrete = write(&path, PhysicsMaterial::new(1.0, 0.1)).unwrap();
    let inline = with_asset(None);
    assert_eq!(effective(&inline), inline.material);
    assert_eq!(named(&inline), None);
    let shared = with_asset(Some(&path));
    assert_eq!(effective(&shared), concrete);
    assert_eq!(named(&shared), Some(key(&path)));
    let dangling = with_asset(Some(&path_in("gone.physmat")));
    assert_eq!(effective(&dangling), PhysicsMaterial::default());
    assert_eq!(named(&dangling), None);
}

#[test]
fn a_deleted_asset_turns_dangling_once_its_removal_is_noted() {
    let path = path_in("deleted.physmat");
    write(&path, PhysicsMaterial::new(0.3, 0.0)).unwrap();
    let before = generation();
    std::fs::remove_file(&path).unwrap();
    // The editor's file watch notes a removed file as it notes a changed one.
    crate::core::fs::written::note_written(&path);
    let shared = with_asset(Some(&path));
    assert!(named(&shared).is_some(), "a lookup alone does not poll");
    poll();
    assert_eq!(named(&shared), None);
    assert!(
        generation() > before,
        "a dropped asset moves the generation"
    );
}

#[test]
fn the_generation_moves_on_a_write_and_holds_otherwise() {
    let path = path_in("tuned.physmat");
    let before = generation();
    write(&path, PhysicsMaterial::new(0.4, 0.0)).unwrap();
    let after = generation();
    assert!(after > before);
    lookup(&path);
    assert_eq!(generation(), after, "a lookup alone changes nothing");
}
