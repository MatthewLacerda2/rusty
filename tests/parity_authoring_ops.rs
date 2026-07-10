//! Editor↔Lua-API convergence over the shared `scene::authoring` ops (#287).
//!
//! Each first-class component whose card was routed through a shared op has its Lua
//! `*.Set*` bindings AND the shared op driven against two sibling entities; the
//! resulting component state must be byte-identical. That pins single-sourcing: the
//! editor card calls the same op the binding does, so the egui panel and the Lua
//! surface can never drift (mirrors `material_library::lua_material_api_and_shared_op_converge`).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::components::{
    AnimatorComponent, CameraComponent, ClearFlags, CollisionDetection, LightComponent, LightType,
    NavMeshAgentComponent, ParticleEmitterComponent, RigidBodyComponent, Tonemap,
    VisualCorrectionComponent,
};
use rusty::navigation::NavigationGraph;
use rusty::render::postfx::QualityPreset;
use rusty::scene::authoring::{
    animator as animator_ops, camera as camera_ops, light as light_ops, nav_agent as nav_ops,
    particles as particle_ops, rigidbody as rb_ops, visual_correction as vc_ops,
};
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// Attach a default-ish light to a fresh entity named `name`; returns its id.
fn entity_with_light(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_light(
        id,
        Some(LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 5.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
        }),
    );
    id
}

#[test]
fn light_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_light(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_light(&mut scene.borrow_mut(), "ViaOp");

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::light::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Light.SetColor({via_lua}, 0.2, 0.4, 0.6)
            Light.SetIntensity({via_lua}, -5.0)
            Light.SetRange({via_lua}, -2.0)
            Light.SetType({via_lua}, "Directional")
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.light_mut(via_op).unwrap();
        light_ops::set_color(&mut e, Vec3::new(0.2, 0.4, 0.6));
        light_ops::set_intensity(&mut e, -5.0); // clamped to 0 by the shared op
        light_ops::set_range(&mut e, -2.0); // clamped to 0 by the shared op
        light_ops::set_type(&mut e, LightType::Directional);
    }

    let sc = scene.borrow();
    let a = sc.world.light(via_lua).unwrap().clone();
    let b = sc.world.light(via_op).unwrap().clone();
    assert_eq!(a.color, b.color);
    assert_eq!(a.intensity, b.intensity);
    assert_eq!(a.range, b.range);
    assert_eq!(a.light_type, b.light_type);
    // The clamp is single-sourced in the op the binding calls.
    assert_eq!(a.intensity, 0.0);
    assert_eq!(a.range, 0.0);
    Ok(())
}

/// Attach a default rigidbody to a fresh entity; returns its id.
fn entity_with_rb(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_rigidbody(
        id,
        Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            use_gravity: true,
            collision_detection: CollisionDetection::Discrete,
        }),
    );
    id
}

#[test]
fn physics_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_rb(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_rb(&mut scene.borrow_mut(), "ViaOp");

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::physics::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Physics.SetVelocity({via_lua}, 1.0, 2.0, 3.0)
            Physics.SetKinematic({via_lua}, true)
            Physics.SetAngularVelocity({via_lua}, 0.0, 4.0, 0.0)
            Physics.SetCollisionDetection({via_lua}, "Continuous")
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.rigidbody_mut(via_op).unwrap();
        rb_ops::set_velocity(&mut e, Vec3::new(1.0, 2.0, 3.0));
        rb_ops::set_kinematic(&mut e, true);
        rb_ops::set_angular_velocity(&mut e, Vec3::new(0.0, 4.0, 0.0));
        rb_ops::set_collision_detection(&mut e, CollisionDetection::Continuous);
    }

    let sc = scene.borrow();
    let a = sc.world.rigidbody(via_lua).unwrap().clone();
    let b = sc.world.rigidbody(via_op).unwrap().clone();
    assert_eq!(a.velocity, b.velocity);
    assert_eq!(a.is_kinematic, b.is_kinematic);
    assert_eq!(a.angular_velocity, b.angular_velocity);
    assert_eq!(a.collision_detection, b.collision_detection);
    Ok(())
}

/// Attach a default nav-agent to a fresh entity; returns its id.
fn entity_with_agent(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_nav_agent(id, Some(NavMeshAgentComponent::default()));
    id
}

#[test]
fn nav_agent_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_agent(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_agent(&mut scene.borrow_mut(), "ViaOp");
    let nav = RefCell::new(NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0));

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::nav::register(&lua, s, &scene, &nav).unwrap();
        lua.load(format!(
            r#"
            NavMeshAgent.SetActive({via_lua}, true)
            NavMeshAgent.SetSpeed({via_lua}, 3.5)
            NavMeshAgent.SetAcceleration({via_lua}, 8.0)
            NavMeshAgent.SetStoppingDistance({via_lua}, 0.5)
            NavMeshAgent.SetRadius({via_lua}, 0.4)
            NavMeshAgent.SetTarget({via_lua}, 1.0, 0.0, 2.0)
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.nav_agent_mut(via_op).unwrap();
        nav_ops::set_active(&mut e, true);
        nav_ops::set_speed(&mut e, 3.5);
        nav_ops::set_acceleration(&mut e, 8.0);
        nav_ops::set_stopping_distance(&mut e, 0.5);
        nav_ops::set_radius(&mut e, 0.4);
        nav_ops::set_target(&mut e, Vec3::new(1.0, 0.0, 2.0));
    }

    let sc = scene.borrow();
    let a = sc.world.nav_agent(via_lua).unwrap().clone();
    let b = sc.world.nav_agent(via_op).unwrap().clone();
    assert_eq!(a.active, b.active);
    assert_eq!(a.speed, b.speed);
    assert_eq!(a.acceleration, b.acceleration);
    assert_eq!(a.stopping_distance, b.stopping_distance);
    assert_eq!(a.radius, b.radius);
    assert_eq!(a.target, b.target);
    Ok(())
}

#[test]
fn animator_stop_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // The only Animator field-write binding is `Stop` (is_playing = false); it and the
    // card's "Is Playing" toggle share `authoring::animator::set_playing`.
    let scene = RefCell::new(Scene::new());
    let console = RefCell::new(ConsoleLogs::new());
    let playing = || AnimatorComponent {
        is_playing: true,
        ..AnimatorComponent::default()
    };
    let via_lua = {
        let mut sc = scene.borrow_mut();
        let id = sc.add_entity("ViaLua".to_string());
        sc.world.set_animator(id, Some(playing()));
        id
    };
    let via_op = {
        let mut sc = scene.borrow_mut();
        let id = sc.add_entity("ViaOp".to_string());
        sc.world.set_animator(id, Some(playing()));
        id
    };

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, &scene, &console).unwrap();
        lua.load(format!("Animator.Stop({via_lua})"))
            .exec()
            .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.animator_mut(via_op).unwrap();
        animator_ops::set_playing(&mut e, false);
    }

    let sc = scene.borrow();
    let a = sc.world.animator(via_lua).unwrap().clone();
    let b = sc.world.animator(via_op).unwrap().clone();
    assert_eq!(a.is_playing, b.is_playing);
    assert!(!a.is_playing);
    Ok(())
}

/// A scene with one active post-FX volume + active camera; returns `(scene_rc, id)`.
fn scene_with_volume_and_cam() -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("PostFx".to_string());
    scene.world.set_visual_correction(
        id,
        Some(VisualCorrectionComponent {
            active: true,
            bloom_active: false,
            bloom_intensity: 1.0,
            bloom_threshold: 1.0,
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            ssr_active: false,
            ssr_quality: "Low".to_string(),
            ssr_temporal_upsampling: false,
            tonemap: Tonemap::Aces,
            gamma: 2.2,
        }),
    );
    scene.world.set_camera(
        id,
        Some(CameraComponent {
            active: true,
            fov: 60.0,
            near: 0.1,
            far: 100.0,
            culling_mask: u32::MAX,
            render_order: 0,
            clear_flags: ClearFlags::Skybox,
            motion_blur_active: false,
            motion_blur_samples: 8,
        }),
    );
    (Rc::new(RefCell::new(scene)), id)
}

/// Apply the same writes the `Graphics.*` script does, via the shared ops directly.
fn apply_graphics_ops(scene: &Rc<RefCell<Scene>>, id: u32) {
    let mut sc = scene.borrow_mut();
    {
        let mut vc = sc.world.visual_correction_mut(id).unwrap();
        vc_ops::set_bloom_active(&mut vc, true);
        vc_ops::set_bloom_intensity(&mut vc, -3.0); // clamped to 0
        vc_ops::set_gamma(&mut vc, -5.0); // clamped to 0.01
        vc_ops::set_tonemap(&mut vc, Tonemap::Reinhard);
        vc_ops::set_ssr_active(&mut vc, true);
        vc_ops::set_ssr_quality(&mut vc, "High".to_string());
    }
    let mut cam = sc.world.camera_mut(id).unwrap();
    camera_ops::set_motion_blur_active(&mut cam, true);
    camera_ops::set_motion_blur_samples(&mut cam, 16);
}

#[test]
fn graphics_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // `Graphics.*` drives the active entity's visual-correction + camera-motion-blur
    // through the SAME ops the editor cards call. We run the bindings against one
    // scene and the ops against a second cloned scene, then compare the components.
    let (lua_scene, lua_id) = scene_with_volume_and_cam();
    let (op_scene, op_id) = scene_with_volume_and_cam();
    let quality = Rc::new(RefCell::new(QualityPreset::High));

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &quality).unwrap();
        lua.load(
            r#"
            Graphics.SetBloomActive(true)
            Graphics.SetBloomIntensity(-3.0)
            Graphics.SetGamma(-5.0)
            Graphics.SetTonemap("reinhard")
            Graphics.SetSsrActive(true)
            Graphics.SetSsrQuality("High")
            Graphics.SetMotionBlurActive(true)
            Graphics.SetMotionBlurSamples(16)
        "#,
        )
        .exec()
        .unwrap();
        Ok(())
    })?;

    apply_graphics_ops(&op_scene, op_id);

    let ls = lua_scene.borrow();
    let os = op_scene.borrow();
    let lvc = ls.world.visual_correction(lua_id).unwrap().clone();
    let ovc = os.world.visual_correction(op_id).unwrap().clone();
    assert_eq!(lvc.bloom_active, ovc.bloom_active);
    assert_eq!(lvc.bloom_intensity, ovc.bloom_intensity);
    assert_eq!(lvc.gamma, ovc.gamma);
    assert_eq!(lvc.tonemap, ovc.tonemap);
    assert_eq!(lvc.ssr_active, ovc.ssr_active);
    assert_eq!(lvc.ssr_quality, ovc.ssr_quality);
    assert_eq!(lvc.bloom_intensity, 0.0, "single-sourced clamp");
    assert_eq!(lvc.gamma, 0.01, "single-sourced clamp");
    let lcam = ls.world.camera(lua_id).unwrap().clone();
    let ocam = os.world.camera(op_id).unwrap().clone();
    assert_eq!(lcam.motion_blur_active, ocam.motion_blur_active);
    assert_eq!(lcam.motion_blur_samples, ocam.motion_blur_samples);
    Ok(())
}

/// Attach a default emitter to a fresh entity; returns its id.
fn entity_with_emitter(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_particles(id, Some(ParticleEmitterComponent::default()));
    id
}

#[test]
fn particles_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // The Particles field-write bindings are `SetActive` and `SetRate` (with its `≥ 0`
    // clamp); both share the ops the card calls.
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_emitter(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_emitter(&mut scene.borrow_mut(), "ViaOp");

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::particle::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Particles.SetActive({via_lua}, false)
            Particles.SetRate({via_lua}, -5.0)
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.particles_mut(via_op).unwrap();
        particle_ops::set_active(&mut e, false);
        particle_ops::set_rate(&mut e, -5.0);
    }

    let sc = scene.borrow();
    let a = sc.world.particles(via_lua).unwrap().clone();
    let b = sc.world.particles(via_op).unwrap().clone();
    assert_eq!(a.active, b.active);
    assert_eq!(a.rate, b.rate);
    assert_eq!(a.rate, 0.0, "single-sourced clamp");
    Ok(())
}
