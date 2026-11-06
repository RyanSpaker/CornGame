// CODEX (GPT-6): real controller/Avian regression checks for loose props and kick boundaries.
use super::*;
use crate::controller::{NoMouse, Player};
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    time::TimeUpdateStrategy,
    window::{CursorOptions, PrimaryWindow},
};
use std::time::Duration;

// CODEX: no renderer/audio device; zero sound volume keeps these physics tests independent of asset loading.
fn scene(mode: KickMode) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::diagnostic::DiagnosticsPlugin,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        PhysicsPlugins::default(),
        crate::controller::plugin,
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        1.0 / 60.0,
    )))
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<AccumulatedMouseMotion>()
    .init_resource::<crate::computer::ComputerSession>()
    .init_resource::<crate::post_it::PostItSession>()
    .insert_resource(NoMouse { on: true })
    .insert_resource(KickSettings {
        mode,
        volume: 0.0,
        ..default()
    });
    app.finish();
    app.cleanup();
    app.world_mut()
        .spawn((PrimaryWindow, CursorOptions::default()));
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 1.0, 20.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let camera = app
        .world_mut()
        .spawn((Player::default(), Transform::from_xyz(0.0, 1.68, 0.0)))
        .id();
    (app, camera)
}

// CODEX: mimic a fitted prop: colliders live on children, including multiple meshes sharing one body.
fn prop(app: &mut App, height: f32, x: f32) -> Entity {
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            GravityScale(0.0),
            Transform::from_xyz(x, height / 2.0 + 0.005, 0.0),
        ))
        .id();
    for z in [-0.035, 0.035] {
        app.world_mut().spawn((
            ChildOf(body),
            Collider::cuboid(0.16, height, 0.09),
            Transform::from_xyz(0.0, 0.0, z),
        ));
    }
    body
}

// CODEX: allow Avian's spatial index to populate before walking toward +X.
fn start(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
}

// CODEX: cover both the ground ray's short-item case and the capsule's tall-item case.
#[test]
fn ignore_walks_through_short_and_tall_child_colliders_without_moving_them() {
    for height in [0.12, 1.0] {
        let (mut app, camera) = scene(KickMode::Ignore);
        let item = prop(&mut app, height, 1.0);
        start(&mut app);
        let original = app.world().get::<Position>(item).unwrap().0;
        for _ in 0..70 {
            app.update();
            let eye = app.world().get::<Transform>(camera).unwrap().translation;
            assert!((eye.y - 1.68).abs() < 0.001, "item became ground: {eye:?}");
        }
        assert!(app.world().get::<Transform>(camera).unwrap().translation.x > 2.5);
        assert!(
            app.world()
                .get::<Position>(item)
                .unwrap()
                .0
                .distance(original)
                < 0.001,
            "Ignore mode still allowed the player collider to push a prop"
        );
    }
}

// CODEX: tiny keys sit beneath the walking capsule but must still receive one bounded, waking kick.
#[test]
fn kick_wakes_small_items_once_per_body_and_cooldown() {
    let (mut app, camera) = scene(KickMode::Kick);
    let item = prop(&mut app, 0.06, 0.65);
    start(&mut app);
    app.world_mut().entity_mut(item).insert(Sleeping);
    let mut kicked = false;
    for _ in 0..30 {
        app.update();
        let velocity = app.world().get::<LinearVelocity>(item).unwrap().0;
        if velocity.x > 1.0 {
            assert!(
                (velocity.x - 2.0).abs() < 0.02,
                "unexpected kick strength: {velocity:?}"
            );
            assert!(!app.world().entity(item).contains::<Sleeping>());
            let spin = app.world().get::<AngularVelocity>(item).unwrap().0.length();
            assert!(
                spin > 1.0 && spin < 2.0,
                "child colliders caused multiple kicks: {spin}"
            );
            // CODEX: keep the prop ahead/in contact so subsequent frames exercise the cooldown.
            for _ in 0..8 {
                app.update();
            }
            assert!(app.world().get::<AngularVelocity>(item).unwrap().0.length() < 2.0);
            kicked = true;
            break;
        }
    }
    assert!(kicked, "the foot sweep missed an item below step height");
    assert!((app.world().get::<Transform>(camera).unwrap().translation.y - 1.68).abs() < 0.001);
}

// CODEX: a generous reach must still stop at a wall instead of shoving an occluded prop.
#[test]
fn kick_sweep_stops_at_static_obstacles() {
    let (mut app, camera) = scene(KickMode::Kick);
    app.world_mut().resource_mut::<KickSettings>().reach = 1.0;
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(0.2, 2.0, 4.0),
        Transform::from_xyz(0.8, 1.0, 0.0),
    ));
    let item = prop(&mut app, 0.12, 1.2);
    start(&mut app);
    for _ in 0..60 {
        app.update();
    }
    assert!(app.world().get::<Transform>(camera).unwrap().translation.x < 0.5);
    assert!(
        app.world().get::<LinearVelocity>(item).unwrap().0.length() < 0.001,
        "kick travelled through a wall"
    );
}
