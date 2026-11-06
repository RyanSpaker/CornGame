// CODEX (GPT-6): mirrored bodies must preserve authored geometry and remain stable across physics frames.
use super::*;
use bevy::time::TimeUpdateStrategy;

// CODEX: the Dell's three negative scale axes previously made physics flip its rotation every frame.
#[test]
fn mirrored_body_keeps_its_parent_and_authored_geometry() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::diagnostic::DiagnosticsPlugin,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        PhysicsPlugins::default(),
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f32(1.0 / 60.0),
    ))
    .add_systems(Update, (insert_body, attach_colliders).chain());
    app.finish();
    app.cleanup();
    let parent = app
        .world_mut()
        .spawn((RigidBody::Static, Transform::IDENTITY))
        .id();
    let authored = Transform::from_xyz(1.9020383, 0.29684693, 1.1905084)
        .with_rotation(Quat::from_xyzw(
            0.42823923, 0.4282392, 0.56268215, 0.56268215,
        ))
        .with_scale(Vec3::new(-2.573662, -2.5736625, -2.5736625));
    let body = app
        .world_mut()
        .spawn((Body(RigidBody::Static), authored, ChildOf(parent)))
        .id();
    app.world_mut().spawn((
        Collider::cuboid(0.2, 0.3, 0.4),
        Transform::IDENTITY,
        ChildOf(body),
    ));
    // CODEX: compare four independent world points, so rotation/scale changes cannot hide behind a fixed origin.
    for _ in 0..120 {
        app.update();
        assert_eq!(app.world().get::<ChildOf>(body).unwrap().parent(), parent);
        let actual = app.world().get::<GlobalTransform>(body).unwrap();
        for probe in [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z] {
            assert!(
                actual
                    .transform_point(probe)
                    .distance(authored.transform_point(probe))
                    < 1e-5,
                "mirrored body changed its authored geometry: {actual:?}"
            );
        }
    }
}
