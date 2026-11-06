//! CODEX (GPT-6.1): pickup-only upright/facing correction; normal pickups preserve world rotation.
use bevy::prelude::*;

use super::Radio;
use crate::{
    controller::Player,
    hold::{Held, HeldRotation},
};

// CODEX: Observe the successful pickup, rather than Interact (which also powers other actions).
pub(super) fn plugin(app: &mut App) {
    app.add_observer(correct_pickup);
}

// CODEX: Capture the player's position once; moving the camera later must not steer the radio.
fn correct_pickup(
    add: On<Add<Held>>,
    radios: Query<(&Radio, &Held, &GlobalTransform)>,
    players: Query<&GlobalTransform, With<Player>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Ok((radio, held, global)) = radios.get(add.entity) else {
        return;
    };
    let up = held.rotation * Vec3::Y;
    // CODEX: Float rounding at exactly the threshold must not trigger a correction.
    if up.dot(Vec3::Y) + 1e-6
        >= radio
            .pickup_tilt_degrees
            .clamp(0.0, 180.0)
            .to_radians()
            .cos()
    {
        return;
    }
    let Some(player) = players.iter().next() else {
        return;
    };
    let to_player = player.translation() - global.translation();
    let forward = Vec3::new(to_player.x, 0.0, to_player.z)
        .try_normalize()
        .or_else(|| {
            // CODEX: Looking straight down at the radio still needs a stable horizontal heading.
            let back = player.rotation() * Vec3::Z;
            Vec3::new(back.x, 0.0, back.z).try_normalize()
        })
        .unwrap_or(Vec3::Z);
    // CODEX: Exported glTF front is +Z, up is +Y. Yaw alone guarantees an upright target.
    let target = Quat::from_rotation_y(forward.x.atan2(forward.z));
    commands.entity(add.entity).insert(HeldRotation {
        target,
        started_at: time.elapsed_secs_f64(),
        seconds: radio.pickup_turn_seconds,
    });
}

// CODEX: Exercise successful-pickup observation and actual carrying without a renderer or audio.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hold::{carry, Holdable};
    use bevy::{
        camera::{CameraProjection, ComputedCameraValues},
        time::TimeUpdateStrategy,
    };
    use std::time::Duration;

    // CODEX: Match the runtime's world-space pickup snapshot with an off-origin carry center.
    fn snapshot(rotation: Quat) -> Held {
        Held {
            rotation,
            scale: Vec3::ONE,
            center: rotation * Vec3::new(0.15, -0.02, 0.05),
            radius: 0.2,
            ..default()
        }
    }

    // CODEX: Pitch, roll and inversion all use the same world-up threshold; ordinary props opt out.
    #[test]
    fn only_tipped_radio_pickups_get_upright_player_facing_target() {
        let mut app = App::new();
        app.init_resource::<Time>().add_plugins(plugin);
        let player = Vec3::new(3.0, 1.6, 2.0);
        let position = Vec3::new(1.0, 0.5, -2.0);
        app.world_mut()
            .spawn((Player::default(), GlobalTransform::from_translation(player)));
        let forward = (player - position).with_y(0.0).normalize();
        for axis in [Vec3::X, Vec3::Z] {
            for degrees in [0.0_f32, 49.9, 50.0, 50.1, 90.0, 180.0] {
                let original =
                    Quat::from_rotation_y(1.1) * Quat::from_axis_angle(axis, degrees.to_radians());
                let item = app
                    .world_mut()
                    .spawn((
                        Radio::default(),
                        GlobalTransform::from_translation(position),
                    ))
                    .id();
                app.world_mut().entity_mut(item).insert(snapshot(original));
                let correction = app.world().get::<HeldRotation>(item);
                assert_eq!(
                    correction.is_some(),
                    degrees > 50.0,
                    "tilt {degrees}, axis {axis:?}"
                );
                if let Some(correction) = correction {
                    assert!((correction.target * Vec3::Y).distance(Vec3::Y) < 1e-5);
                    assert!((correction.target * Vec3::Z).distance(forward) < 1e-5);
                    assert_eq!(correction.seconds, 0.7);
                }
            }
        }
        let ordinary = app
            .world_mut()
            .spawn((
                GlobalTransform::IDENTITY,
                snapshot(Quat::from_rotation_z(1.6)),
            ))
            .id();
        assert!(app.world().get::<HeldRotation>(ordinary).is_none());
    }

    // CODEX: Actual carry honors the captured target under a parent even after the player turns.
    #[test]
    fn carry_finishes_once_and_does_not_keep_steering_with_player() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, plugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, carry);
        let camera = Camera {
            computed: ComputedCameraValues {
                clip_from_view: PerspectiveProjection::default().get_clip_from_view(),
                ..default()
            },
            ..default()
        };
        let player = app
            .world_mut()
            .spawn((
                camera,
                Player::default(),
                Transform::from_xyz(0.0, 1.6, 0.0),
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn(
                Transform::from_xyz(4.0, 0.0, 1.0)
                    .with_rotation(Quat::from_rotation_y(0.8))
                    .with_scale(Vec3::splat(1.5)),
            )
            .id();
        let original = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let item = app
            .world_mut()
            .spawn((
                Radio {
                    pickup_turn_seconds: 0.4,
                    ..default()
                },
                Holdable::default(),
                Transform::from_xyz(0.0, 1.0, -2.0).with_rotation(original),
            ))
            .id();
        app.update();
        let parent_global = *app.world().get::<GlobalTransform>(parent).unwrap();
        let local = app
            .world()
            .get::<GlobalTransform>(item)
            .unwrap()
            .reparented_to(&parent_global);
        app.world_mut()
            .entity_mut(item)
            .insert((local, ChildOf(parent)));
        app.world_mut().entity_mut(item).insert(snapshot(original));
        let target = app.world().get::<HeldRotation>(item).unwrap().target;
        assert_eq!(app.world().get::<HeldRotation>(item).unwrap().seconds, 0.4);
        app.update();
        let halfway = app.world().get::<GlobalTransform>(item).unwrap().rotation();
        assert!(halfway.angle_between(original) > 0.01 && halfway.angle_between(target) > 0.01);
        *app.world_mut().get_mut::<Transform>(player).unwrap() =
            Transform::from_xyz(3.0, 1.6, 1.0).with_rotation(Quat::from_rotation_y(1.7));
        for _ in 0..10 {
            app.update();
        }
        let pose = app.world().get::<GlobalTransform>(item).unwrap();
        assert!(
            pose.rotation().angle_between(target) < 1e-3,
            "finished target stays fixed in world space"
        );
        assert_eq!(
            app.world().get::<Held>(item).unwrap().rotation,
            original,
            "cached bounds keep original basis"
        );
    }

    // CODEX: A radio directly beneath the player still gets a finite upright target.
    #[test]
    fn overhead_pickup_has_stable_heading() {
        let mut app = App::new();
        app.init_resource::<Time>().add_plugins(plugin);
        app.world_mut().spawn((
            Player::default(),
            GlobalTransform::from(
                Transform::from_xyz(0.0, 2.0, 0.0)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ),
        ));
        let item = app
            .world_mut()
            .spawn((
                Radio::default(),
                GlobalTransform::IDENTITY,
                snapshot(Quat::from_rotation_x(std::f32::consts::PI)),
            ))
            .id();
        let target = app.world().get::<HeldRotation>(item).unwrap().target;
        assert!(target.is_finite());
        assert!((target * Vec3::Y).distance(Vec3::Y) < 1e-5);
    }
}
