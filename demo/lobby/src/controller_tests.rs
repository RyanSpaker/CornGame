// CODEX (GPT-6): regression checks for wall climbing and stepping onto furniture.
use super::*;
use bevy::time::TimeUpdateStrategy;

// CODEX: run the actual walking system with Avian and deterministic input, without opening a window.
#[test]
fn walls_and_tables_block_climbing_but_inclines_remain_walkable() {
    for obstacle in ["curved wall", "table", "ramp"] {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::diagnostic::DiagnosticsPlugin,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            PhysicsPlugins::default(),
            crate::kick::plugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1.0 / 60.0,
        )))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<ComputerSession>()
        .init_resource::<crate::post_it::PostItSession>()
        .insert_resource(NoMouse { on: true })
        .add_systems(Update, (spawn_body, walk).chain());
        app.finish();
        app.cleanup();
        app.world_mut()
            .spawn((PrimaryWindow, CursorOptions::default()));
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 1.0, 20.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        // CODEX: stacked round logs reproduce the lobby walls; the box reproduces a tabletop edge.
        if obstacle == "curved wall" {
            for row in 0..6 {
                app.world_mut().spawn((
                    RigidBody::Static,
                    Collider::capsule(0.3, 8.0),
                    Transform::from_xyz(2.0, 0.25 + row as f32 * 0.5, 0.0)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
            }
        } else if obstacle == "table" {
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(1.0, 0.8, 4.0),
                Transform::from_xyz(2.0, 0.4, 0.0),
            ));
        } else {
            // CODEX: a 30 degree incline rises from the floor into the walking path.
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(6.0, 0.2, 4.0),
                Transform::from_xyz(3.0, 1.4, 0.0)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::PI / 6.0)),
            ));
        }
        let camera = app
            .world_mut()
            .spawn((Player::default(), Transform::from_xyz(0.0, 1.68, 0.0)))
            .id();
        // CODEX: let spatial queries populate before holding forward against the obstacle for five seconds.
        for _ in 0..3 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        let mut highest = 1.68_f32;
        for _ in 0..300 {
            app.update();
            let eye = app.world().get::<Transform>(camera).unwrap().translation;
            highest = highest.max(eye.y);
            if obstacle != "ramp" {
                assert!(
                    (eye.y - 1.68).abs() < 0.001,
                    "{obstacle}: wall contact changed height to {eye:?}"
                );
            }
        }
        let eye = app.world().get::<Transform>(camera).unwrap().translation;
        if obstacle == "ramp" {
            assert!(
                highest > 3.0 && eye.x > 4.0,
                "incline should remain walkable: highest={highest}, eye={eye:?}"
            );
        } else {
            assert!(
                eye.x > 1.0 && eye.x < 1.6,
                "{obstacle}: obstacle should stop forward motion: {eye:?}"
            );
        }
    }
}
