#[test]
fn a_nan_probe_position_is_no_edit_on_an_idle_frame() {
    let mut scene = crate::Scene::new();
    scene.probes.add_probe(glam::Vec3::NAN);
    scene.begin_edits();
    scene.pause_edits();
    assert!(!scene.commit_edits(None), "NaN never equals itself");
}

#[test]
fn marking_a_probe_manual_undoes() {
    let mut scene = crate::Scene::new();
    scene.probes.add_probe(glam::Vec3::ONE);
    scene.begin_edits();
    scene.probes.set_manual(0, true);
    assert!(scene.commit_edits(None), "the flag is an edit");
    scene.undo().expect("a step to undo");
    assert!(!scene.probes.probes()[0].manual);
}

#[test]
fn undo_after_a_re_placing_bake_matches_cubes_by_position() {
    use glam::Vec3;
    let mut scene = crate::Scene::new();
    for x in [0.0, 1.0] {
        let i = scene.reflection_probes.add(Vec3::X * x, Vec3::ONE);
        scene.reflection_probes.set_cubemap(i, format!("a{i}.ktx2"));
    }
    scene.begin_edits();
    scene.reflection_probes.edit(0, |p| p.position = Vec3::Y);
    assert!(scene.commit_edits(None));
    // The bake: probe 1's position first, then a new one, each with a new cube.
    let mut baked = crate::ReflectionProbeSet::default();
    for (i, x) in [1.0, 5.0].into_iter().enumerate() {
        baked.add(Vec3::X * x, Vec3::ONE);
        baked.set_cubemap(i, format!("b{i}.ktx2"));
    }
    scene.reflection_probes = baked;
    scene.undo().expect("a step to undo");
    let paths: Vec<&str> = (scene.reflection_probes.probes.iter())
        .map(|p| p.cubemap_path.as_str())
        .collect();
    assert_eq!(paths, ["a0.ktx2", "b0.ktx2"]);
}
