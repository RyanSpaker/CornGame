//! CODEX (GPT-6): shared one-shot upright/facing turns for tipped pickups and entering tuning.
use bevy::prelude::*;

use super::Radio;
use crate::{
    controller::Player,
    hold::{Held, HeldRotation},
};

// CODEX: Observe the successful pickup, rather than Interact (which also powers other actions).
pub(super) fn plugin(app: &mut App) {
    app.add_observer(correct_pickup)
        .add_observer(face_for_tuning);
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
    commands.entity(add.entity).insert(HeldRotation {
        from: None,
        target: facing_target(global, player),
        started_at: time.elapsed_secs_f64(),
        seconds: radio.pickup_turn_seconds,
    });
}

// CODEX: Tuning a held radio asks for a turn regardless of tilt; placed radios keep their pose.
#[derive(EntityEvent)]
pub(super) struct FaceRadio {
    pub entity: Entity,
}

// CODEX: Capture the current interpolated rotation so re-entering tuning mid-turn cannot snap.
fn face_for_tuning(
    on: On<FaceRadio>,
    radios: Query<(&Radio, &GlobalTransform, &Held, Option<&HeldRotation>)>,
    players: Query<&GlobalTransform, With<Player>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Ok((radio, global, held, correction)) = radios.get(on.entity) else {
        return;
    };
    let Some(player) = players.iter().next() else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let from = correction.map_or(held.rotation, |turn| turn.sample(held.rotation, now));
    commands.entity(on.entity).insert(HeldRotation {
        from: Some(from),
        target: tuning_target(global, player),
        started_at: now,
        seconds: radio.pickup_turn_seconds,
    });
}

// CODEX (GPT-6): T faces the full player direction without roll; pickup correction stays yaw-only.
fn tuning_target(global: &GlobalTransform, player: &GlobalTransform) -> Quat {
    let to_player = player.translation() - global.translation();
    let pitch = -to_player.y.atan2(to_player.x.hypot(to_player.z));
    facing_target(global, player) * Quat::from_rotation_x(pitch)
}

// CODEX: Exported glTF front is +Z, up is +Y; yaw alone makes the radio upright and player-facing.
fn facing_target(global: &GlobalTransform, player: &GlobalTransform) -> Quat {
    let to_player = player.translation() - global.translation();
    let forward = Vec3::new(to_player.x, 0.0, to_player.z)
        .try_normalize()
        .or_else(|| {
            // CODEX: Looking straight down still needs a stable horizontal heading.
            let back = player.rotation() * Vec3::Z;
            Vec3::new(back.x, 0.0, back.z).try_normalize()
        })
        .unwrap_or(Vec3::Z);
    Quat::from_rotation_y(forward.x.atan2(forward.z))
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
                    assert_eq!(correction.seconds, 0.5);
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
        // CODEX: The shared delay preserves the initial orientation before the turn starts.
        assert!(
            app.world()
                .get::<GlobalTransform>(item)
                .unwrap()
                .rotation()
                .angle_between(original)
                < 1e-3
        );
        app.update();
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

    // CODEX: Re-entering tuning during a held pickup correction begins at its current eased pose.
    #[test]
    fn tuning_retargets_held_radio_without_snapping_to_pickup_rotation() {
        let mut app = App::new();
        app.init_resource::<Time>().add_plugins(plugin);
        let player = app
            .world_mut()
            .spawn((
                Player::default(),
                GlobalTransform::from_translation(Vec3::Z),
            ))
            .id();
        let original = Quat::from_rotation_z(1.6);
        let item = app
            .world_mut()
            .spawn((Radio::default(), GlobalTransform::IDENTITY))
            .id();
        app.world_mut().entity_mut(item).insert(snapshot(original));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(350));
        let current = app
            .world()
            .get::<HeldRotation>(item)
            .unwrap()
            .sample(original, 0.35);
        *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
            GlobalTransform::from_translation(Vec3::X);
        app.world_mut().trigger(FaceRadio { entity: item });
        app.world_mut().flush();
        let turn = app.world().get::<HeldRotation>(item).unwrap();
        assert!(turn.sample(original, 0.35).angle_between(current) < 1e-3);
        assert!((turn.target * Vec3::Z).distance(Vec3::X) < 1e-5);
        assert_eq!(app.world().get::<Held>(item).unwrap().rotation, original);
    }

    // CODEX (GPT-6): T includes pitch above/below the radio, including vertical views, without roll.
    #[test]
    fn tuning_faces_player_with_pitch_while_pickup_stays_upright() {
        let mut app = App::new();
        app.init_resource::<Time>().add_plugins(plugin);
        let player = app
            .world_mut()
            .spawn((Player::default(), GlobalTransform::IDENTITY))
            .id();
        let position = Vec3::new(1.0, 0.5, -2.0);
        for offset in [
            Vec3::new(3.0, 1.5, 2.0),
            Vec3::new(-2.0, -1.0, 3.0),
            Vec3::Y,
            Vec3::NEG_Y,
        ] {
            *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
                GlobalTransform::from_translation(position + offset);
            let item = app
                .world_mut()
                .spawn((
                    Radio::default(),
                    GlobalTransform::from_translation(position),
                ))
                .id();
            app.world_mut()
                .entity_mut(item)
                .insert(snapshot(Quat::from_rotation_z(1.6)));
            let pickup = app.world().get::<HeldRotation>(item).unwrap().target;
            assert!((pickup * Vec3::Y).distance(Vec3::Y) < 1e-5);
            app.world_mut().trigger(FaceRadio { entity: item });
            app.world_mut().flush();
            let turn = app.world().get::<HeldRotation>(item).unwrap();
            assert!(turn.target.is_finite());
            assert!((turn.target * Vec3::Z).distance(offset.normalize()) < 1e-5);
            assert!(
                (turn.target * Vec3::X).y.abs() < 1e-5,
                "the radio's horizontal edges stay level"
            );
        }
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
